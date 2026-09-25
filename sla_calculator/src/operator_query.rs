use soroban_sdk::{symbol_short, Address, Env, Symbol};

const OPERATOR_KEY: Symbol = symbol_short!("OPERATOR");

#[derive(Debug, Clone, PartialEq)]
pub enum QueryError {
    NotInitialized,
}

/// Returns the current operator address, or `NotInitialized` if unset.
pub fn get_operator(env: &Env) -> Result<Address, QueryError> {
    env.storage()
        .instance()
        .get(&OPERATOR_KEY)
        .ok_or(QueryError::NotInitialized)
}
