use soroban_sdk::{symbol_short, Env, Symbol};

const REENTRANCY_FLAG_KEY: Symbol = symbol_short!("REENTER");

#[derive(Debug, Clone, PartialEq)]
pub enum ReentrancyError {
    ReentrancyDetected,
}

/// Sets the reentrancy flag, returning an error if it is already set.
pub fn enter(env: &Env) -> Result<(), ReentrancyError> {
    if env.storage().temporary().has(&REENTRANCY_FLAG_KEY) {
        return Err(ReentrancyError::ReentrancyDetected);
    }
    env.storage().temporary().set(&REENTRANCY_FLAG_KEY, &true);
    Ok(())
}

/// Clears the reentrancy flag on exit from the guarded call.
pub fn exit(env: &Env) {
    env.storage().temporary().remove(&REENTRANCY_FLAG_KEY);
}
