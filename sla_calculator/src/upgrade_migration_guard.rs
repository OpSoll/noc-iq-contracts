// Contract upgrade path WASM hash validator / migration compatibility
// check. Complements upgrade_path.rs and storage_migration.rs (#1008 / #1009).
use soroban_sdk::{contracterror, BytesN, Env};

use crate::storage_migration::{
    recover_interrupted_migration, stored_version, MigrationError, CURRENT_STORAGE_VERSION,
};
use crate::storage_version::{require_compatible_storage_version, EXPECTED_STORAGE_VERSION};
use crate::SLAError;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum UpgradeGuardError {
    IncompatibleLayout = 1,
    MigrationRequired = 2,
}

/// Blocks a WASM upgrade unless the target hash is on the
/// pre-verified-compatible allowlist (populated by an off-chain
/// migration dry-run before the upgrade is proposed).
pub fn assert_upgrade_compatible(
    target_wasm_hash: &BytesN<32>,
    verified_compatible_hashes: &[BytesN<32>],
) -> Result<(), UpgradeGuardError> {
    if verified_compatible_hashes.contains(target_wasm_hash) {
        Ok(())
    } else {
        Err(UpgradeGuardError::IncompatibleLayout)
    }
}

/// Gate entrypoints when on-chain storage is from a newer binary (#1008).
pub fn assert_storage_not_from_future(env: &Env) -> Result<(), UpgradeGuardError> {
    match require_compatible_storage_version(env) {
        Ok(_) => Ok(()),
        Err(SLAError::VersionMismatch) => Err(UpgradeGuardError::MigrationRequired),
        Err(_) => Err(UpgradeGuardError::MigrationRequired),
    }
}

/// Resume an interrupted storage migration after upgrade (#1009).
pub fn resume_storage_migration(env: &Env) -> Result<(), UpgradeGuardError> {
    match recover_interrupted_migration(env) {
        Ok(()) => Ok(()),
        Err(MigrationError::AlreadyComplete) => Ok(()),
        Err(MigrationError::VersionMismatch) => Err(UpgradeGuardError::MigrationRequired),
        Err(MigrationError::ConversionFailed) => Err(UpgradeGuardError::IncompatibleLayout),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage_migration::{set_legacy_keys, set_phase, PHASE_CONVERTING};
    use crate::SLACalculatorContract;
    use soroban_sdk::Symbol;

    fn hash(env: &Env, byte: u8) -> BytesN<32> {
        BytesN::from_array(env, &[byte; 32])
    }

    #[test]
    fn test_verified_hash_is_allowed() {
        let env = Env::default();
        let allow = [hash(&env, 1), hash(&env, 2)];
        assert_eq!(assert_upgrade_compatible(&hash(&env, 2), &allow), Ok(()));
    }

    #[test]
    fn test_unverified_hash_is_blocked() {
        let env = Env::default();
        let allow = [hash(&env, 1)];
        assert_eq!(
            assert_upgrade_compatible(&hash(&env, 9), &allow),
            Err(UpgradeGuardError::IncompatibleLayout)
        );
    }

    #[test]
    fn test_empty_allowlist_blocks_every_migration() {
        let env = Env::default();
        assert_eq!(
            assert_upgrade_compatible(&hash(&env, 1), &[]),
            Err(UpgradeGuardError::IncompatibleLayout)
        );
    }

    #[test]
    fn test_hash_differing_in_one_byte_is_blocked() {
        let env = Env::default();
        let mut near = [1u8; 32];
        near[31] = 2;
        let allow = [hash(&env, 1)];
        assert_eq!(
            assert_upgrade_compatible(&BytesN::from_array(&env, &near), &allow),
            Err(UpgradeGuardError::IncompatibleLayout)
        );
    }

    #[test]
    fn test_future_storage_version_blocks_with_migration_required() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&id, || {
            env.storage().instance().set(
                &soroban_sdk::symbol_short!("VER"),
                &(EXPECTED_STORAGE_VERSION + 3),
            );
            assert_eq!(
                assert_storage_not_from_future(&env),
                Err(UpgradeGuardError::MigrationRequired)
            );
        });
    }

    #[test]
    fn test_resume_after_interrupted_phases() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&id, || {
            env.storage()
                .instance()
                .set(&soroban_sdk::symbol_short!("MIGVER"), &0u32);
            let k = Symbol::new(&env, "x");
            set_legacy_keys(&env, &soroban_sdk::vec![&env, k]);
            set_phase(&env, PHASE_CONVERTING);

            assert_eq!(resume_storage_migration(&env), Ok(()));
            assert_eq!(stored_version(&env), CURRENT_STORAGE_VERSION);

            // Second resume is a no-op success (already complete).
            assert_eq!(resume_storage_migration(&env), Ok(()));
        });
    }
}
