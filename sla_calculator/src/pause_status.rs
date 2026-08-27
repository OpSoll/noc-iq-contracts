use soroban_sdk::{contracttype, symbol_short, Address, Env, String, Symbol};

const PAUSED_KEY: Symbol = symbol_short!("PAUSED");
const PAUSE_INFO_KEY: Symbol = symbol_short!("PAUSEINF");

#[contracttype]
#[derive(Clone)]
pub struct PauseInfo {
    pub reason: String,
    pub paused_at: u64,
    pub paused_by: Address,
}

/// Returns whether the contract is currently paused.
pub fn is_paused(env: &Env) -> bool {
    env.storage().instance().get(&PAUSED_KEY).unwrap_or(false)
}

/// Returns the stored pause metadata, if the contract is paused.
pub fn get_pause_info(env: &Env) -> Option<PauseInfo> {
    env.storage().instance().get(&PAUSE_INFO_KEY)
}
