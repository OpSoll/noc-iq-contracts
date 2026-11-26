use soroban_sdk::{contracttype, symbol_short, Address, Env, Map, Symbol, Vec};

use crate::{SLAError, ADMIN_KEY};

// -----------------------------------------------------------------------
// Storage keys
// -----------------------------------------------------------------------
const OPERATORS_KEY: Symbol = symbol_short!("OPER");
/// Membership epoch: bumped on every effective add/remove (#1013).
const MEMBER_EPOCH_KEY: Symbol = symbol_short!("OPEPOCH");
/// Epoch at which each operator last joined the set (#1013).
const JOINED_EPOCH_KEY: Symbol = symbol_short!("OPJOIN");

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
    let mut joined = Map::<Address, u32>::new(env);
    joined.set(operator.clone(), 0);
    env.storage().instance().set(&MEMBER_EPOCH_KEY, &0u32);
    env.storage().instance().set(&JOINED_EPOCH_KEY, &joined);
}

/// Current membership epoch. Starts at 0 and increases by one on every
/// effective operator addition or removal, so any vote weight recorded
/// against an older epoch can be detected as stale (#1013).
pub fn membership_epoch(env: &Env) -> u32 {
    env.storage().instance().get(&MEMBER_EPOCH_KEY).unwrap_or(0)
}

/// Epoch at which `operator` most recently joined the operator set, or
/// `None` if it is not currently an operator (#1013).
pub fn joined_epoch(env: &Env, operator: &Address) -> Option<u32> {
    let state: OperatorState = env.storage().instance().get(&OPERATORS_KEY)?;
    if !state.operators.get(operator.clone()).unwrap_or(false) {
        return None;
    }
    let joined: Map<Address, u32> = env
        .storage()
        .instance()
        .get(&JOINED_EPOCH_KEY)
        .unwrap_or(Map::new(env));
    Some(joined.get(operator.clone()).unwrap_or(0))
}

fn bump_membership_epoch(env: &Env) -> u32 {
    let next = membership_epoch(env).saturating_add(1);
    env.storage().instance().set(&MEMBER_EPOCH_KEY, &next);
    next
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
pub fn add_operator(env: &Env, caller: &Address, new_operator: &Address) -> Result<(), SLAError> {
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

    let epoch = bump_membership_epoch(env);
    let mut joined: Map<Address, u32> = env
        .storage()
        .instance()
        .get(&JOINED_EPOCH_KEY)
        .unwrap_or(Map::new(env));
    joined.set(new_operator.clone(), epoch);
    env.storage().instance().set(&JOINED_EPOCH_KEY, &joined);

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
pub fn remove_operator(env: &Env, caller: &Address, operator: &Address) -> Result<(), SLAError> {
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
    bump_membership_epoch(env);

    env.events()
        .publish((EVENT_OP_REM, EVENT_VERSION, caller), (operator.clone(),));

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
