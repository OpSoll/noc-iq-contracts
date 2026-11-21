use soroban_sdk::{symbol_short, Env, Symbol};

use crate::SLAError;

/// Storage layout version this binary reads and writes.
pub const EXPECTED_STORAGE_VERSION: u32 = 1;

const STORAGE_VERSION_KEY: Symbol = symbol_short!("VER");
const MIGRATION_FLAG_KEY: Symbol = symbol_short!("MIGRATED");
/// Sample state keys used to prove future-version rejection is non-destructive (#1008).
const BALANCE_KEY: Symbol = symbol_short!("BAL");
const CONFIG_KEY: Symbol = symbol_short!("CFG");

pub fn read_storage_version(env: &Env) -> Option<u32> {
    env.storage().instance().get(&STORAGE_VERSION_KEY)
}

/// Reads the storage version and asserts it matches
/// `EXPECTED_STORAGE_VERSION`. An unset version is treated as the current
/// one (fresh contract); anything else fails fast with
/// `SLAError::VersionMismatch`.
///
/// Unknown **future** versions must not reset balances, config, or history (#1008).
pub fn assert_storage_version(env: &Env) -> Result<u32, SLAError> {
    match read_storage_version(env) {
        None => Ok(EXPECTED_STORAGE_VERSION),
        Some(v) if v == EXPECTED_STORAGE_VERSION => Ok(v),
        Some(_) => Err(SLAError::VersionMismatch),
    }
}

/// Like [`assert_storage_version`], but distinguishes future versions explicitly
/// for upgrade/migration callers (#1008).
pub fn require_compatible_storage_version(env: &Env) -> Result<u32, SLAError> {
    match read_storage_version(env) {
        None => Ok(EXPECTED_STORAGE_VERSION),
        Some(v) if v == EXPECTED_STORAGE_VERSION => Ok(v),
        Some(v) if v > EXPECTED_STORAGE_VERSION => Err(SLAError::VersionMismatch),
        Some(_) => Err(SLAError::VersionMismatch),
    }
}

pub fn is_migration_complete(env: &Env) -> bool {
    env.storage()
        .instance()
        .get::<Symbol, bool>(&MIGRATION_FLAG_KEY)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    #[test]
    fn test_storage_version_unset_returns_none() {
        let env = Env::default();
        assert_eq!(read_storage_version(&env), None);
    }

    #[test]
    fn test_migration_flag_defaults_to_false() {
        let env = Env::default();
        assert!(!is_migration_complete(&env));
    }

    fn with_contract<R>(env: &Env, f: impl FnOnce() -> R) -> R {
        let id = env.register_contract(None, crate::SLACalculatorContract);
        env.as_contract(&id, f)
    }

    #[test]
    fn test_assert_storage_version_accepts_unset_and_current() {
        let env = Env::default();
        with_contract(&env, || {
            assert_eq!(assert_storage_version(&env), Ok(EXPECTED_STORAGE_VERSION));
            env.storage()
                .instance()
                .set(&STORAGE_VERSION_KEY, &EXPECTED_STORAGE_VERSION);
            assert_eq!(assert_storage_version(&env), Ok(EXPECTED_STORAGE_VERSION));
        });
    }

    #[test]
    fn test_assert_storage_version_rejects_mismatch() {
        let env = Env::default();
        with_contract(&env, || {
            env.storage().instance().set(&STORAGE_VERSION_KEY, &99u32);
            assert_eq!(assert_storage_version(&env), Err(SLAError::VersionMismatch));
        });
    }

    /// #1008 — future version returns migration-required error; state untouched.
    #[test]
    fn test_future_version_rejected_without_destructive_reset() {
        let env = Env::default();
        with_contract(&env, || {
            let future = EXPECTED_STORAGE_VERSION + 5;
            env.storage().instance().set(&STORAGE_VERSION_KEY, &future);
            env.storage().instance().set(&BALANCE_KEY, &1_000i128);
            env.storage().instance().set(&CONFIG_KEY, &42u32);

            assert_eq!(
                require_compatible_storage_version(&env),
                Err(SLAError::VersionMismatch)
            );
            assert_eq!(assert_storage_version(&env), Err(SLAError::VersionMismatch));

            // Version and business state must be unchanged (no silent reset).
            assert_eq!(read_storage_version(&env), Some(future));
            assert_eq!(
                env.storage().instance().get::<_, i128>(&BALANCE_KEY),
                Some(1_000)
            );
            assert_eq!(
                env.storage().instance().get::<_, u32>(&CONFIG_KEY),
                Some(42)
            );
        });
    }
}
