// Contract configuration freeze toggle. Complements config_freeze.rs.
use soroban_sdk::{contracterror, symbol_short, Address, Env, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ConfigFreezeError {
    ConfigIsFrozen = 1,
}

const FROZEN_KEY: Symbol = symbol_short!("CFGFRZN");

/// Admin permanently locks configuration settings. Irreversible.
pub fn freeze_configuration(env: &Env, admin: &Address) {
    admin.require_auth();
    env.storage().instance().set(&FROZEN_KEY, &true);
}

/// Call before any config mutation; rejects once frozen.
pub fn assert_config_not_frozen(env: &Env) -> Result<(), ConfigFreezeError> {
    let frozen: bool = env.storage().instance().get(&FROZEN_KEY).unwrap_or(false);
    if frozen {
        Err(ConfigFreezeError::ConfigIsFrozen)
    } else {
        Ok(())
    }
}
