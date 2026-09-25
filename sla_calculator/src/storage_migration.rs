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
