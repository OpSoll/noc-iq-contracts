use soroban_sdk::{symbol_short, Env, Symbol};

const PAUSED_KEY: Symbol = symbol_short!("PAUSED");

#[derive(Debug, Clone, PartialEq)]
pub enum GuardError {
    ContractPaused,
}

/// Call at the top of every state-mutating entry point to block execution
/// while the contract is paused.
pub fn require_not_paused(env: &Env) -> Result<(), GuardError> {
    let paused: bool = env.storage().instance().get(&PAUSED_KEY).unwrap_or(false);
    if paused {
        return Err(GuardError::ContractPaused);
    }
    Ok(())
}
