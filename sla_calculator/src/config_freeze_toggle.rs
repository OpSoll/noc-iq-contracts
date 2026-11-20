// Contract configuration freeze toggle. Complements config_freeze.rs.
use soroban_sdk::{contracterror, symbol_short, Address, Env, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ConfigFreezeError {
    ConfigIsFrozen = 1,
}

const FROZEN_KEY: Symbol = symbol_short!("CFGFRZN");
const FROZEN_EVENT: Symbol = symbol_short!("cfgfrz");
const UNFROZEN_EVENT: Symbol = symbol_short!("cfgunfrz");

pub fn freeze_configuration(env: &Env, admin: &Address) {
    admin.require_auth();
    if !is_configuration_frozen(env) {
        env.storage().instance().set(&FROZEN_KEY, &true);
        env.events().publish((FROZEN_EVENT, admin.clone()), true);
    }
}

pub fn unfreeze_configuration(env: &Env, admin: &Address) {
    admin.require_auth();
    if is_configuration_frozen(env) {
        env.storage().instance().set(&FROZEN_KEY, &false);
        env.events()
            .publish((UNFROZEN_EVENT, admin.clone()), false);
    }
}

pub fn is_configuration_frozen(env: &Env) -> bool {
    env.storage().instance().get(&FROZEN_KEY).unwrap_or(false)
}

/// Call before any config mutation; rejects once frozen.
pub fn assert_config_not_frozen(env: &Env) -> Result<(), ConfigFreezeError> {
    if is_configuration_frozen(env) {
        Err(ConfigFreezeError::ConfigIsFrozen)
    } else {
        Ok(())
    }
}
