use soroban_sdk::{symbol_short, Address, Env, Symbol};

use crate::{SLAError, ADMIN_KEY};

// -----------------------------------------------------------------------
// Storage keys
// -----------------------------------------------------------------------
const WHITELIST_KEY: Symbol = symbol_short!("WL");
/// Issue #703: per-operator role assignments for the permission matrix.
const OPERATOR_ROLES_KEY: Symbol = symbol_short!("OP_ROLE");

// -----------------------------------------------------------------------
// Events
// -----------------------------------------------------------------------
const EVENT_WL_ADD: Symbol = symbol_short!("wl_add");
const EVENT_WL_REM: Symbol = symbol_short!("wl_rem");
const EVENT_WL_CLR: Symbol = symbol_short!("wl_clr");
const EVENT_VERSION: Symbol = symbol_short!("v1");
/// Issue #703.
const EVENT_ROLE_ASSIGNED: Symbol = symbol_short!("role_set");
const EVENT_ROLE_REVOKED: Symbol = symbol_short!("role_rvk");

// -----------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------

/// Cross-contract whitelist state.
#[soroban_sdk::contracttype]
pub struct WhitelistState {
    /// Set of whitelisted contract addresses.
    pub contracts: soroban_sdk::Map<Address, bool>,
    /// Number of whitelisted contracts.
    pub count: u32,
    /// Whether whitelist checking is enabled.
    /// When disabled, all contracts are allowed.
    pub enforcement_enabled: bool,
}

/// Issue #703: operator role in the permission matrix. Roles are strict
/// and non-overlapping — e.g. an `Admin` does not implicitly gain
/// `Reporter` or `Auditor` privileges; each guard checks for its own
/// specific role.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperatorRole {
    /// May submit outage events.
    Reporter,
    /// May submit disputes.
    Auditor,
    /// Contract administrator (config mutations, whitelist management).
    Admin,
    /// May trigger emergency pause/freeze controls.
    EmergencyGuard,
}

// -----------------------------------------------------------------------
// Functions
// -----------------------------------------------------------------------

/// Initialize the whitelist as empty with enforcement disabled.
pub fn init_whitelist(env: &Env) {
    env.storage().instance().set(
        &WHITELIST_KEY,
        &WhitelistState {
            contracts: soroban_sdk::Map::new(env),
            count: 0,
            enforcement_enabled: false,
        },
    );
}

/// Add a contract address to the whitelist (admin only).
///
/// # Arguments
/// - `caller`: Must be the current admin.
/// - `contract_address`: Address to whitelist.
///
/// # Events
/// - `wl_add`: Emitted with the whitelisted address.
pub fn add_to_whitelist(
    env: &Env,
    caller: &Address,
    contract_address: &Address,
) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    let mut state: WhitelistState =
        env.storage()
            .instance()
            .get(&WHITELIST_KEY)
            .unwrap_or(WhitelistState {
                contracts: soroban_sdk::Map::new(env),
                count: 0,
                enforcement_enabled: false,
            });

    if state
        .contracts
        .get(contract_address.clone())
        .unwrap_or(false)
    {
        return Ok(()); // Already whitelisted (idempotent)
    }

    state.contracts.set(contract_address.clone(), true);
    state.count = state.count.saturating_add(1);
    env.storage().instance().set(&WHITELIST_KEY, &state);

    env.events().publish(
        (EVENT_WL_ADD, EVENT_VERSION, caller),
        (contract_address.clone(),),
    );

    Ok(())
}

/// Remove a contract address from the whitelist (admin only).
///
/// # Arguments
/// - `caller`: Must be the current admin.
/// - `contract_address`: Address to remove.
///
/// # Events
/// - `wl_rem`: Emitted with the removed address.
pub fn remove_from_whitelist(
    env: &Env,
    caller: &Address,
    contract_address: &Address,
) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    let mut state: WhitelistState =
        env.storage()
            .instance()
            .get(&WHITELIST_KEY)
            .unwrap_or(WhitelistState {
                contracts: soroban_sdk::Map::new(env),
                count: 0,
                enforcement_enabled: false,
            });

    if !state
        .contracts
        .get(contract_address.clone())
        .unwrap_or(false)
    {
        return Ok(()); // Not in whitelist (idempotent)
    }

    state.contracts.set(contract_address.clone(), false);
    state.count = state.count.saturating_sub(1);
    env.storage().instance().set(&WHITELIST_KEY, &state);

    env.events().publish(
        (EVENT_WL_REM, EVENT_VERSION, caller),
        (contract_address.clone(),),
    );

    Ok(())
}

/// Clear the entire whitelist (admin only).
///
/// # Events
/// - `wl_clr`: Emitted when whitelist is cleared.
pub fn clear_whitelist(env: &Env, caller: &Address) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    env.storage().instance().set(
        &WHITELIST_KEY,
        &WhitelistState {
            contracts: soroban_sdk::Map::new(env),
            count: 0,
            enforcement_enabled: false,
        },
    );

    env.events()
        .publish((EVENT_WL_CLR, EVENT_VERSION, caller), ());

    Ok(())
}

/// Enable whitelist enforcement (admin only).
///
/// When enabled, only whitelisted contracts can interact with this contract.
pub fn enable_whitelist(env: &Env, caller: &Address) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    let mut state: WhitelistState =
        env.storage()
            .instance()
            .get(&WHITELIST_KEY)
            .unwrap_or(WhitelistState {
                contracts: soroban_sdk::Map::new(env),
                count: 0,
                enforcement_enabled: false,
            });

    state.enforcement_enabled = true;
    env.storage().instance().set(&WHITELIST_KEY, &state);

    Ok(())
}

/// Disable whitelist enforcement (admin only).
///
/// When disabled, all contracts are allowed.
pub fn disable_whitelist(env: &Env, caller: &Address) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    let mut state: WhitelistState =
        env.storage()
            .instance()
            .get(&WHITELIST_KEY)
            .unwrap_or(WhitelistState {
                contracts: soroban_sdk::Map::new(env),
                count: 0,
                enforcement_enabled: false,
            });

    state.enforcement_enabled = false;
    env.storage().instance().set(&WHITELIST_KEY, &state);

    Ok(())
}

/// Check if a contract address is whitelisted (or if enforcement is disabled).
///
/// Returns Ok(()) if allowed, or Unauthorized if enforcement is on and
/// the contract is not in the whitelist.
pub fn check_whitelist(env: &Env, contract_address: &Address) -> Result<(), SLAError> {
    let state: WhitelistState = match env.storage().instance().get(&WHITELIST_KEY) {
        Some(s) => s,
        None => return Ok(()), // No whitelist = allow all
    };

    if !state.enforcement_enabled {
        return Ok(()); // Enforcement off = allow all
    }

    if state
        .contracts
        .get(contract_address.clone())
        .unwrap_or(false)
    {
        Ok(())
    } else {
        Err(SLAError::Unauthorized)
    }
}

/// Check if a contract address is in the whitelist (without enforcement check).
pub fn is_whitelisted(env: &Env, contract_address: &Address) -> Result<bool, SLAError> {
    let state: WhitelistState = match env.storage().instance().get(&WHITELIST_KEY) {
        Some(s) => s,
        None => return Ok(false),
    };

    Ok(state
        .contracts
        .get(contract_address.clone())
        .unwrap_or(false))
}

/// Returns the whitelist status and count.
pub fn get_whitelist_status(env: &Env) -> Result<(bool, u32), SLAError> {
    let state: WhitelistState = match env.storage().instance().get(&WHITELIST_KEY) {
        Some(s) => s,
        None => return Ok((false, 0)),
    };

    Ok((state.enforcement_enabled, state.count))
}

/// Returns all whitelisted contract addresses.
pub fn list_whitelisted(env: &Env) -> Result<soroban_sdk::Vec<Address>, SLAError> {
    let state: WhitelistState = match env.storage().instance().get(&WHITELIST_KEY) {
        Some(s) => s,
        None => return Ok(soroban_sdk::Vec::new(env)),
    };

    let mut result = soroban_sdk::Vec::new(env);
    for (addr, active) in state.contracts.iter() {
        if active {
            result.push_back(addr);
        }
    }
    Ok(result)
}

/// Helper to verify admin role.
fn require_admin(env: &Env, caller: &Address) -> Result<(), SLAError> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&ADMIN_KEY)
        .ok_or(SLAError::NotInitialized)?;
    if *caller != admin {
        return Err(SLAError::Unauthorized);
    }
    Ok(())
}

// -----------------------------------------------------------------------
// Issue #703: operator role-based permission matrix
// -----------------------------------------------------------------------

fn load_operator_roles(env: &Env) -> soroban_sdk::Map<Address, OperatorRole> {
    env.storage()
        .instance()
        .get(&OPERATOR_ROLES_KEY)
        .unwrap_or_else(|| soroban_sdk::Map::new(env))
}

/// Admin-only: assign `operator` the given `role`, replacing any prior
/// assignment for that address.
///
/// # Events
/// - `role_set`: Emitted with the operator and their new role.
pub fn assign_role(
    env: &Env,
    caller: &Address,
    operator: &Address,
    role: OperatorRole,
) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    let mut roles = load_operator_roles(env);
    roles.set(operator.clone(), role.clone());
    env.storage().instance().set(&OPERATOR_ROLES_KEY, &roles);

    env.events().publish(
        (EVENT_ROLE_ASSIGNED, EVENT_VERSION, caller),
        (operator.clone(), role),
    );

    Ok(())
}

/// Admin-only: remove any role assignment for `operator`.
///
/// # Events
/// - `role_rvk`: Emitted with the operator whose role was revoked.
pub fn revoke_role(env: &Env, caller: &Address, operator: &Address) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    let mut roles = load_operator_roles(env);
    roles.remove(operator.clone());
    env.storage().instance().set(&OPERATOR_ROLES_KEY, &roles);

    env.events().publish(
        (EVENT_ROLE_REVOKED, EVENT_VERSION, caller),
        operator.clone(),
    );

    Ok(())
}

/// Returns `operator`'s currently assigned role, if any.
pub fn get_operator_role(env: &Env, operator: &Address) -> Option<OperatorRole> {
    load_operator_roles(env).get(operator.clone())
}

/// Issue #703 acceptance criterion: strict role check — returns `Ok(())`
/// only if `operator` is assigned exactly `expected_role`. Roles do not
/// imply one another (e.g. `Admin` does not pass a `Reporter` check).
///
/// # Errors
/// Returns `Unauthorized` if `operator` has no role assigned, or a
/// different role than `expected_role`.
pub fn require_role(
    env: &Env,
    operator: &Address,
    expected_role: OperatorRole,
) -> Result<(), SLAError> {
    match get_operator_role(env, operator) {
        Some(role) if role == expected_role => Ok(()),
        _ => Err(SLAError::Unauthorized),
    }
}

/// Issue #703 acceptance criterion: a `Reporter` may submit outage
/// events. Guards the outage-submission entry point.
pub fn require_reporter(env: &Env, operator: &Address) -> Result<(), SLAError> {
    require_role(env, operator, OperatorRole::Reporter)
}

/// Issue #703 acceptance criterion: an `Auditor` may submit disputes.
/// Guards the dispute-filing entry point.
pub fn require_auditor(env: &Env, operator: &Address) -> Result<(), SLAError> {
    require_role(env, operator, OperatorRole::Auditor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn setup(env: &Env, admin: &Address) {
        env.storage().instance().set(&ADMIN_KEY, admin);
    }

    #[test]
    fn test_admin_can_assign_and_revoke_role() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register_contract(None, crate::SLACalculatorContract);
        let admin = Address::generate(&env);
        let operator = Address::generate(&env);

        env.as_contract(&cid, || {
            setup(&env, &admin);
            assert_eq!(get_operator_role(&env, &operator), None);
        });
        env.as_contract(&cid, || {
            assign_role(&env, &admin, &operator, OperatorRole::Reporter).unwrap();
        });
        env.as_contract(&cid, || {
            assert_eq!(
                get_operator_role(&env, &operator),
                Some(OperatorRole::Reporter)
            );
        });
        env.as_contract(&cid, || {
            revoke_role(&env, &admin, &operator).unwrap();
        });
        env.as_contract(&cid, || {
            assert_eq!(get_operator_role(&env, &operator), None);
        });
    }

    #[test]
    fn test_non_admin_cannot_assign_role() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register_contract(None, crate::SLACalculatorContract);
        let admin = Address::generate(&env);
        let not_admin = Address::generate(&env);
        let operator = Address::generate(&env);

        env.as_contract(&cid, || {
            setup(&env, &admin);
            let result = assign_role(&env, &not_admin, &operator, OperatorRole::Admin);
            assert_eq!(result, Err(SLAError::Unauthorized));
        });
    }

    #[test]
    fn test_reporter_can_only_submit_outage_events() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register_contract(None, crate::SLACalculatorContract);
        let admin = Address::generate(&env);
        let reporter = Address::generate(&env);
        let auditor = Address::generate(&env);

        env.as_contract(&cid, || {
            setup(&env, &admin);
            assign_role(&env, &admin, &reporter, OperatorRole::Reporter).unwrap();
            assign_role(&env, &admin, &auditor, OperatorRole::Auditor).unwrap();
        });
        env.as_contract(&cid, || {
            assert!(require_reporter(&env, &reporter).is_ok());
            assert_eq!(
                require_reporter(&env, &auditor),
                Err(SLAError::Unauthorized)
            );
        });
    }

    #[test]
    fn test_auditor_can_submit_disputes() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register_contract(None, crate::SLACalculatorContract);
        let admin = Address::generate(&env);
        let reporter = Address::generate(&env);
        let auditor = Address::generate(&env);

        env.as_contract(&cid, || {
            setup(&env, &admin);
            assign_role(&env, &admin, &reporter, OperatorRole::Reporter).unwrap();
            assign_role(&env, &admin, &auditor, OperatorRole::Auditor).unwrap();
        });
        env.as_contract(&cid, || {
            assert!(require_auditor(&env, &auditor).is_ok());
            assert_eq!(
                require_auditor(&env, &reporter),
                Err(SLAError::Unauthorized)
            );
        });
    }

    #[test]
    fn test_role_check_is_strict_admin_does_not_imply_reporter() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register_contract(None, crate::SLACalculatorContract);
        let admin = Address::generate(&env);
        let super_operator = Address::generate(&env);

        env.as_contract(&cid, || {
            setup(&env, &admin);
            assign_role(&env, &admin, &super_operator, OperatorRole::Admin).unwrap();
        });
        env.as_contract(&cid, || {
            assert_eq!(
                require_reporter(&env, &super_operator),
                Err(SLAError::Unauthorized)
            );
            assert_eq!(
                require_auditor(&env, &super_operator),
                Err(SLAError::Unauthorized)
            );
        });
    }

    #[test]
    fn test_unassigned_operator_fails_role_check() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register_contract(None, crate::SLACalculatorContract);
        let admin = Address::generate(&env);
        let stranger = Address::generate(&env);

        env.as_contract(&cid, || {
            setup(&env, &admin);
            assert_eq!(
                require_reporter(&env, &stranger),
                Err(SLAError::Unauthorized)
            );
        });
    }
}
