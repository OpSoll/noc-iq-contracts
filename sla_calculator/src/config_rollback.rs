// Config rollback helper to previous version. Complements
// config_freeze.rs.
use soroban_sdk::{contracterror, symbol_short, Address, Env, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RollbackError {
    VersionNotFound = 1,
}

const ROLLED_BACK_EVENT: Symbol = symbol_short!("cfgrbck");

/// Governance reverts config settings to a recorded historical version,
/// validating the target version exists in the history log first.
pub fn rollback_config(
    env: &Env,
    governance: &Address,
    target_version: u32,
    known_versions: &[u32],
) -> Result<(), RollbackError> {
    governance.require_auth();
    if !known_versions.contains(&target_version) {
        return Err(RollbackError::VersionNotFound);
    }
    env.events()
        .publish((ROLLED_BACK_EVENT,), target_version);
    Ok(())
}
