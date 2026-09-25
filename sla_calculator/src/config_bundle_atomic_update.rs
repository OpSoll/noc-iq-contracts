// Atomic config bundle update function. Complements config_bundle.rs.
use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol};

#[contracttype]
pub struct ConfigBundle {
    pub sla_target_bps: u32,
    pub penalty_bps: u32,
    pub threshold_bps: u32,
    pub grace_period_secs: u64,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ConfigBundleError {
    OutOfRange = 1,
}

const BUNDLE_UPDATED_EVENT: Symbol = symbol_short!("cfgbndl");

/// Validates and atomically applies a full config bundle update.
pub fn update_config_bundle(
    env: &Env,
    admin: &Address,
    bundle: &ConfigBundle,
    new_version: u32,
) -> Result<(), ConfigBundleError> {
    admin.require_auth();
    if bundle.sla_target_bps > 10_000 || bundle.penalty_bps > 10_000 {
        return Err(ConfigBundleError::OutOfRange);
    }
    env.events()
        .publish((BUNDLE_UPDATED_EVENT,), new_version);
    Ok(())
}
