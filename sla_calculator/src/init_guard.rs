use soroban_sdk::{symbol_short, Env, Symbol};

const ADMIN_KEY: Symbol = symbol_short!("ADMIN");

#[derive(Debug, Clone, PartialEq)]
pub enum InitError {
    AlreadyInitialized,
}

/// Guards `initialize()` against being called more than once.
pub fn ensure_not_initialized(env: &Env) -> Result<(), InitError> {
    if env.storage().instance().has(&ADMIN_KEY) {
        return Err(InitError::AlreadyInitialized);
    }
    Ok(())
}
