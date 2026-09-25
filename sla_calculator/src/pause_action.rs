use soroban_sdk::{contracttype, symbol_short, Address, Env, String, Symbol};

const PAUSED_KEY: Symbol = symbol_short!("PAUSED");
const PAUSE_INFO_KEY: Symbol = symbol_short!("PAUSEINF");
const CONTRACT_PAUSED_EVENT: Symbol = symbol_short!("paused");

#[contracttype]
#[derive(Clone)]
pub struct PauseInfo {
    pub reason: String,
    pub paused_at: u64,
    pub paused_by: Address,
}

/// Admin-only: pauses the contract with an explanatory reason.
pub fn pause(env: &Env, admin: &Address, reason: String) {
    admin.require_auth();
    let info = PauseInfo {
        reason,
        paused_at: env.ledger().timestamp(),
        paused_by: admin.clone(),
    };
    env.storage().instance().set(&PAUSED_KEY, &true);
    env.storage().instance().set(&PAUSE_INFO_KEY, &info);
    env.events()
        .publish((CONTRACT_PAUSED_EVENT,), admin.clone());
}
