use soroban_sdk::{symbol_short, Address, Env, Symbol};

const PAUSED_KEY: Symbol = symbol_short!("PAUSED");
const PAUSE_INFO_KEY: Symbol = symbol_short!("PAUSEINF");
const CONTRACT_UNPAUSED_EVENT: Symbol = symbol_short!("unpaused");

/// Admin-only: clears the paused flag and pause metadata.
pub fn unpause(env: &Env, admin: &Address) {
    admin.require_auth();
    env.storage().instance().set(&PAUSED_KEY, &false);
    env.storage().instance().remove(&PAUSE_INFO_KEY);
    env.events()
        .publish((CONTRACT_UNPAUSED_EVENT,), admin.clone());
}
