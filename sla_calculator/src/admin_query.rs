use soroban_sdk::{symbol_short, Address, Env, Symbol};

const ADMIN_KEY: Symbol = symbol_short!("ADMIN");

#[derive(Debug, Clone, PartialEq)]
pub enum QueryError {
    NotInitialized,
}

/// Returns the current admin address, or `NotInitialized` if unset.
pub fn get_admin(env: &Env) -> Result<Address, QueryError> {
    env.storage()
        .instance()
        .get(&ADMIN_KEY)
        .ok_or(QueryError::NotInitialized)
}
