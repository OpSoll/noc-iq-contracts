#![no_std]

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Map, Symbol};

use crate::{SLAError, ADMIN_KEY};

// -----------------------------------------------------------------------
// Storage keys
// -----------------------------------------------------------------------
const OPERATORS_KEY: Symbol = symbol_short!("OPER");
const PENDING_OP_KEY: Symbol = symbol_short!("POP");

// -----------------------------------------------------------------------
// Events
// -----------------------------------------------------------------------
const EVENT_OP_ADD: Symbol = symbol_short!("op_add");
const EVENT_OP_REM: Symbol = symbol_short!("op_rem");
const EVENT_VERSION: Symbol = symbol_short!("v1");

// -----------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------

/// Multi-operator state stored in contract storage.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperatorState {
    /// Set of active operator addresses.
    pub operators: Map<Address, bool>,
    /// Number of active operators.
    pub count: u32,
}

// -----------------------------------------------------------------------
// Functions
// -----------------------------------------------------------------------

/// Initialize multi-operator storage with a single default operator.
pub fn init_operators(env: &Env, operator: &Address) {
    let mut operators = Map::<Address, bool>::new(env);
    operators.set(operator.clone(), true);
    env.storage().instance().set(
        &OPERATORS_KEY,
        &OperatorState {
            operators,
            count: 1,
        },
    );
}

/// Check if the caller is an authorized operator.
pub fn is_operator(env: &Env, caller: &Address) -> Result<(), SLAError> {
    let state: OperatorState = env
        .storage()
        .instance()
        .get(&OPERATORS_KEY)
        .ok_or(SLAError::NotInitialized)?;
    if state.operators.get(caller.clone()).unwrap_or(false) {
        Ok(())
    } else {
        Err(SLAError::Unauthorized)
    }
}

/// Add a new operator (admin only).
///
/// # Arguments
/// - `caller`: Must be the current admin.
/// - `new_operator`: Address to add as an operator.
///
/// # Events
/// - `op_add`: Emitted with the new operator address.
///
/// # Errors
/// Returns `Unauthorized` if caller is not admin.
pub fn add_operator(
    env: &Env,
    caller: &Address,
    new_operator: &Address,
) -> Result<(), SLAError> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&ADMIN_KEY)
        .ok_or(SLAError::NotInitialized)?;
    if *caller != admin {
        return Err(SLAError::Unauthorized);
    }

    let mut state: OperatorState = env
        .storage()
        .instance()
        .get(&OPERATORS_KEY)
        .ok_or(SLAError::NotInitialized)?;

    // Idempotent: if already an operator, no-op
    if state.operators.get(new_operator.clone()).unwrap_or(false) {
        return Ok(());
    }

    state.operators.set(new_operator.clone(), true);
    state.count = state.count.saturating_add(1);
    env.storage().instance().set(&OPERATORS_KEY, &state);

    env.events().publish(
        (EVENT_OP_ADD, EVENT_VERSION, caller),
        (new_operator.clone(),),
    );

    Ok(())
}

/// Remove an operator (admin only).
///
/// # Arguments
/// - `caller`: Must be the current admin.
/// - `operator`: Address to remove.
///
/// # Events
/// - `op_rem`: Emitted with the removed operator address.
///
/// # Errors
/// Returns `Unauthorized` if caller is not admin.
pub fn remove_operator(
    env: &Env,
    caller: &Address,
    operator: &Address,
) -> Result<(), SLAError> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&ADMIN_KEY)
        .ok_or(SLAError::NotInitialized)?;
    if *caller != admin {
        return Err(SLAError::Unauthorized);
    }

    let mut state: OperatorState = env
        .storage()
        .instance()
        .get(&OPERATORS_KEY)
        .ok_or(SLAError::NotInitialized)?;

    // Idempotent: if not an operator, no-op
    if !state.operators.get(operator.clone()).unwrap_or(false) {
        return Ok(());
    }

    state.operators.set(operator.clone(), false);
    state.count = state.count.saturating_sub(1);
    env.storage().instance().set(&OPERATORS_KEY, &state);

    env.events().publish(
        (EVENT_OP_REM, EVENT_VERSION, caller),
        (operator.clone(),),
    );

    Ok(())
}

/// Returns the list of active operator addresses.
pub fn list_operators(env: &Env) -> Result<Vec<Address>, SLAError> {
    let state: OperatorState = env
        .storage()
        .instance()
        .get(&OPERATORS_KEY)
        .ok_or(SLAError::NotInitialized)?;

    let mut result = Vec::new(env);
    for (addr, active) in state.operators.iter() {
        if active {
            result.push_back(addr);
        }
    }
    Ok(result)
}

/// Returns the number of active operators.
pub fn operator_count(env: &Env) -> Result<u32, SLAError> {
    let state: OperatorState = env
        .storage()
        .instance()
        .get(&OPERATORS_KEY)
        .ok_or(SLAError::NotInitialized)?;
    Ok(state.count)
}
