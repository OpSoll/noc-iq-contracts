// Re-entrancy lock guard on mutating calls. Complements
// cross_contract_safety.rs.
use soroban_sdk::{contracterror, symbol_short, Env, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ReentrancyError {
    ReentrantCall = 1,
}

const LOCK_KEY: Symbol = symbol_short!("REENTR");

/// Sets the re-entrancy lock; rejects the call if already locked.
pub fn enter(env: &Env) -> Result<(), ReentrancyError> {
    let locked: bool = env
        .storage()
        .instance()
        .get(&LOCK_KEY)
        .unwrap_or(false);
    if locked {
        return Err(ReentrancyError::ReentrantCall);
    }
    env.storage().instance().set(&LOCK_KEY, &true);
    Ok(())
}

/// Clears the re-entrancy lock at the end of the mutating function.
pub fn exit(env: &Env) {
    env.storage().instance().set(&LOCK_KEY, &false);
}
