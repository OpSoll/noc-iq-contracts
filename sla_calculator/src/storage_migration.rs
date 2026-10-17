use soroban_sdk::{symbol_short, Env, Symbol};

const STORAGE_VERSION_KEY: Symbol = symbol_short!("MIGVER");
pub const CURRENT_STORAGE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq)]
pub enum MigrationError {
    VersionMismatch,
}

/// Reads the stored schema version, defaulting to version 1 if unset.
pub fn stored_version(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&STORAGE_VERSION_KEY)
        .unwrap_or(1)
}

/// Runs sequential migration steps up to `CURRENT_STORAGE_VERSION`.
/// Rejects on-chain versions newer than what this binary supports.
pub fn migrate_storage(env: &Env) -> Result<(), MigrationError> {
    let version = stored_version(env);
    if version > CURRENT_STORAGE_VERSION {
        return Err(MigrationError::VersionMismatch);
    }
    // Future migration steps would run here, version by version.
    env.storage()
        .instance()
        .set(&STORAGE_VERSION_KEY, &CURRENT_STORAGE_VERSION);
    Ok(())
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
            // Running it again changes nothing and still succeeds.
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
            // The rejected migration must not downgrade the stored version.
            assert_eq!(stored_version(&env), CURRENT_STORAGE_VERSION + 1);
        });
    }

    #[test]
    fn test_unset_version_defaults_to_one() {
        let env = Env::default();
        let id = contract(&env);
        env.as_contract(&id, || assert_eq!(stored_version(&env), 1));
    }
}
