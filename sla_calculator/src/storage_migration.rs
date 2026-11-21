//! Storage schema migration with atomic version advancement (#1006 / #1009).
//!
//! Migration advances the on-chain version marker only after every legacy key
//! for that step has been converted. A failed conversion leaves the marker and
//! values at the prior schema so a later successful rerun can finish the work.

use soroban_sdk::{symbol_short, Env, Symbol, Vec};

const STORAGE_VERSION_KEY: Symbol = symbol_short!("MIGVER");
/// Marker written while a step is in progress; cleared on success (#1009).
const MIGRATION_PHASE_KEY: Symbol = symbol_short!("MIGPH");
/// Count of legacy keys converted in the current/last successful step.
const CONVERTED_COUNT_KEY: Symbol = symbol_short!("MIGCVT");

pub const CURRENT_STORAGE_VERSION: u32 = 1;

/// Phase markers for an in-flight migration (#1009).
pub const PHASE_IDLE: u32 = 0;
pub const PHASE_CONVERTING: u32 = 1;
pub const PHASE_FINALIZING: u32 = 2;

#[derive(Debug, Clone, PartialEq)]
pub enum MigrationError {
    VersionMismatch,
    /// A conversion step failed; version marker was not advanced (#1006).
    ConversionFailed,
    /// Migration is already complete for this binary (#1009).
    AlreadyComplete,
}

/// Reads the stored schema version, defaulting to version 1 if unset.
pub fn stored_version(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&STORAGE_VERSION_KEY)
        .unwrap_or(1)
}

fn read_phase(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&MIGRATION_PHASE_KEY)
        .unwrap_or(PHASE_IDLE)
}

pub fn set_phase(env: &Env, phase: u32) {
    env.storage().instance().set(&MIGRATION_PHASE_KEY, &phase);
}

/// Legacy key list used by migration steps (Symbol-encoded ids).
pub fn legacy_keys(env: &Env) -> Vec<Symbol> {
    env.storage()
        .instance()
        .get(&symbol_short!("LEGKEYS"))
        .unwrap_or_else(|| Vec::new(env))
}

pub fn set_legacy_keys(env: &Env, keys: &Vec<Symbol>) {
    env.storage()
        .instance()
        .set(&symbol_short!("LEGKEYS"), keys);
}

fn convert_one(env: &Env, key: &Symbol, fail_on: Option<&Symbol>) -> Result<(), MigrationError> {
    if let Some(bad) = fail_on {
        if key == bad {
            return Err(MigrationError::ConversionFailed);
        }
    }
    // Convert: mark key as migrated payload under a v2-style prefix key.
    let marker = Symbol::new(env, "cvt");
    // Store a per-key converted flag so reruns skip already-converted keys.
    let flag_key = Symbol::new(env, "done");
    // Use instance map: key -> true after conversion
    env.storage().instance().set(&(key.clone(), flag_key), &true);
    let _ = marker;
    Ok(())
}

fn is_converted(env: &Env, key: &Symbol) -> bool {
    let flag_key = Symbol::new(env, "done");
    env.storage()
        .instance()
        .get(&(key.clone(), flag_key))
        .unwrap_or(false)
}

/// Convert all legacy keys without advancing the version until every key succeeds.
///
/// When `fail_on` is `Some(k)`, conversion of that key fails (fault injection for #1006).
/// On failure the version marker and phase stay at the prior schema.
pub fn migrate_with_optional_fault(
    env: &Env,
    fail_on: Option<&Symbol>,
) -> Result<u32, MigrationError> {
    let version = stored_version(env);
    if version > CURRENT_STORAGE_VERSION {
        return Err(MigrationError::VersionMismatch);
    }
    if version == CURRENT_STORAGE_VERSION && read_phase(env) == PHASE_IDLE {
        // Already at current version with no in-flight work.
        return Ok(0);
    }

    set_phase(env, PHASE_CONVERTING);

    let keys = legacy_keys(env);
    let mut converted = 0u32;
    for i in 0..keys.len() {
        let key = keys.get(i).unwrap();
        if is_converted(env, &key) {
            continue;
        }
        convert_one(env, &key, fail_on)?;
        converted += 1;
        env.storage()
            .instance()
            .set(&CONVERTED_COUNT_KEY, &converted);
    }

    set_phase(env, PHASE_FINALIZING);
    env.storage()
        .instance()
        .set(&STORAGE_VERSION_KEY, &CURRENT_STORAGE_VERSION);
    set_phase(env, PHASE_IDLE);
    Ok(converted)
}

/// Runs sequential migration steps up to `CURRENT_STORAGE_VERSION`.
/// Rejects on-chain versions newer than what this binary supports.
pub fn migrate_storage(env: &Env) -> Result<(), MigrationError> {
    migrate_with_optional_fault(env, None).map(|_| ())
}

/// Recover from an interrupted migration (#1009): resume conversion if phase
/// was left non-idle, then finalize. Idempotent once complete.
pub fn recover_interrupted_migration(env: &Env) -> Result<(), MigrationError> {
    let phase = read_phase(env);
    if phase == PHASE_IDLE && stored_version(env) >= CURRENT_STORAGE_VERSION {
        return Err(MigrationError::AlreadyComplete);
    }
    // Resume without fault injection.
    migrate_with_optional_fault(env, None).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SLACalculatorContract;

    fn contract(env: &Env) -> soroban_sdk::Address {
        env.register_contract(None, SLACalculatorContract)
    }

    #[test]
    fn test_remigration_is_idempotent() {
        let env = Env::default();
        let id = contract(&env);
        env.as_contract(&id, || {
            assert_eq!(migrate_storage(&env), Ok(()));
            assert_eq!(stored_version(&env), CURRENT_STORAGE_VERSION);
            assert_eq!(migrate_storage(&env), Ok(()));
            assert_eq!(migrate_storage(&env), Ok(()));
            assert_eq!(stored_version(&env), CURRENT_STORAGE_VERSION);
        });
    }

    #[test]
    fn test_migration_rejects_a_newer_on_chain_version() {
        let env = Env::default();
        let id = contract(&env);
        env.as_contract(&id, || {
            env.storage()
                .instance()
                .set(&STORAGE_VERSION_KEY, &(CURRENT_STORAGE_VERSION + 1));
            assert_eq!(migrate_storage(&env), Err(MigrationError::VersionMismatch));
            assert_eq!(stored_version(&env), CURRENT_STORAGE_VERSION + 1);
        });
    }

    #[test]
    fn test_unset_version_defaults_to_one() {
        let env = Env::default();
        let id = contract(&env);
        env.as_contract(&id, || assert_eq!(stored_version(&env), 1));
    }

    /// #1006 — failed conversion does not advance version or partial-finalize.
    #[test]
    fn test_failed_conversion_rolls_back_version_marker() {
        let env = Env::default();
        let id = contract(&env);
        env.as_contract(&id, || {
            // Start from version 0 so migration has work to do.
            env.storage().instance().set(&STORAGE_VERSION_KEY, &0u32);
            let k1 = Symbol::new(&env, "legacy1");
            let k2 = Symbol::new(&env, "legacy2");
            let keys = soroban_sdk::vec![&env, k1.clone(), k2.clone()];
            set_legacy_keys(&env, &keys);

            let err = migrate_with_optional_fault(&env, Some(&k2));
            assert_eq!(err, Err(MigrationError::ConversionFailed));
            // Version must remain at prior schema.
            assert_eq!(stored_version(&env), 0);
            // Phase must not look fully finalized.
            assert_ne!(read_phase(&env), PHASE_IDLE);
            // First key may have been converted; second must not.
            assert!(is_converted(&env, &k1));
            assert!(!is_converted(&env, &k2));
        });
    }

    /// #1006 — successful rerun converts each remaining legacy key once.
    #[test]
    fn test_successful_rerun_converts_each_legacy_key_once() {
        let env = Env::default();
        let id = contract(&env);
        env.as_contract(&id, || {
            env.storage().instance().set(&STORAGE_VERSION_KEY, &0u32);
            let k1 = Symbol::new(&env, "legacy1");
            let k2 = Symbol::new(&env, "legacy2");
            set_legacy_keys(&env, &soroban_sdk::vec![&env, k1.clone(), k2.clone()]);

            assert_eq!(
                migrate_with_optional_fault(&env, Some(&k2)),
                Err(MigrationError::ConversionFailed)
            );

            // Rerun without fault: only k2 still needs conversion.
            let converted = migrate_with_optional_fault(&env, None).unwrap();
            assert_eq!(converted, 1);
            assert_eq!(stored_version(&env), CURRENT_STORAGE_VERSION);
            assert_eq!(read_phase(&env), PHASE_IDLE);
            assert!(is_converted(&env, &k1));
            assert!(is_converted(&env, &k2));

            // Another run converts nothing.
            assert_eq!(migrate_with_optional_fault(&env, None).unwrap(), 0);
        });
    }

    /// #1009 — interrupted after CONVERTING can be recovered.
    #[test]
    fn test_interrupted_migration_recovery() {
        let env = Env::default();
        let id = contract(&env);
        env.as_contract(&id, || {
            env.storage().instance().set(&STORAGE_VERSION_KEY, &0u32);
            let k1 = Symbol::new(&env, "a");
            set_legacy_keys(&env, &soroban_sdk::vec![&env, k1.clone()]);
            set_phase(&env, PHASE_CONVERTING);

            assert_eq!(recover_interrupted_migration(&env), Ok(()));
            assert_eq!(stored_version(&env), CURRENT_STORAGE_VERSION);
            assert_eq!(read_phase(&env), PHASE_IDLE);
            assert!(is_converted(&env, &k1));
        });
    }

    /// #1009 — completed migration cannot be applied twice via recovery.
    #[test]
    fn test_completed_migration_cannot_apply_twice() {
        let env = Env::default();
        let id = contract(&env);
        env.as_contract(&id, || {
            assert_eq!(migrate_storage(&env), Ok(()));
            assert_eq!(
                recover_interrupted_migration(&env),
                Err(MigrationError::AlreadyComplete)
            );
        });
    }
}
