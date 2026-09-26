use soroban_sdk::{symbol_short, Address, Env, Symbol};

use crate::{SLAError, ADMIN_KEY};

// -----------------------------------------------------------------------
// Storage keys
// -----------------------------------------------------------------------
const WHITELIST_KEY: Symbol = symbol_short!("WL");
/// Issue #703: per-operator role assignments for the permission matrix.
const OPERATOR_ROLES_KEY: Symbol = symbol_short!("OP_ROLE");
/// Issue #672: Service onboarding registration records.
const SERVICE_REG_KEY: Symbol = symbol_short!("SVC_REG");
/// Issue #672: Global configured grace period duration in seconds.
const GRACE_PERIOD_KEY: Symbol = symbol_short!("GRACE_P");
const DEFAULT_GRACE_PERIOD_SECONDS: u64 = 14 * 86_400; // 14 days

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
/// Issue #672.
const EVENT_SVC_REG: Symbol = symbol_short!("svc_reg");
const EVENT_GRACE_EXEMPT: Symbol = symbol_short!("grc_exm");
const EVENT_GRACE_SET: Symbol = symbol_short!("grc_set");

// -----------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------

/// Issue #672: Service onboarding registration record.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceRegistrationRecord {
    pub service: Address,
    pub registered_at: u64,
    pub grace_period_seconds: u64,
}

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

// -----------------------------------------------------------------------
// Issue #672: Service onboarding grace period
// -----------------------------------------------------------------------

/// Configure the default onboarding grace period duration (admin only).
pub fn set_grace_period_duration(
    env: &Env,
    caller: &Address,
    duration_seconds: u64,
) -> Result<(), SLAError> {
    require_admin(env, caller)?;
    env.storage()
        .instance()
        .set(&GRACE_PERIOD_KEY, &duration_seconds);
    env.events()
        .publish((EVENT_GRACE_SET, EVENT_VERSION, caller), duration_seconds);
    Ok(())
}

/// Returns the current onboarding grace period duration in seconds (default 14 days).
pub fn get_grace_period_duration(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&GRACE_PERIOD_KEY)
        .unwrap_or(DEFAULT_GRACE_PERIOD_SECONDS)
}

fn load_service_registrations(
    env: &Env,
) -> soroban_sdk::Map<Address, ServiceRegistrationRecord> {
    env.storage()
        .instance()
        .get(&SERVICE_REG_KEY)
        .unwrap_or_else(|| soroban_sdk::Map::new(env))
}

/// Register a service with an onboarding grace period (admin only).
/// Stores service registration timestamp in instance storage.
pub fn register_service(
    env: &Env,
    caller: &Address,
    service: &Address,
) -> Result<ServiceRegistrationRecord, SLAError> {
    require_admin(env, caller)?;

    let grace_period_seconds = get_grace_period_duration(env);
    let registered_at = env.ledger().timestamp();
    let record = ServiceRegistrationRecord {
        service: service.clone(),
        registered_at,
        grace_period_seconds,
    };

    let mut map = load_service_registrations(env);
    map.set(service.clone(), record.clone());
    env.storage().instance().set(&SERVICE_REG_KEY, &map);

    env.events().publish(
        (EVENT_SVC_REG, EVENT_VERSION, caller),
        (service.clone(), registered_at, grace_period_seconds),
    );

    Ok(record)
}

/// Returns the registration record for `service`, if any.
pub fn get_service_registration(
    env: &Env,
    service: &Address,
) -> Option<ServiceRegistrationRecord> {
    load_service_registrations(env).get(service.clone())
}

/// Returns true if `service` is currently in its onboarding grace period.
/// Automatically expires once the ledger timestamp is at or past `registered_at + grace_period_seconds`.
pub fn is_service_in_grace_period(env: &Env, service: &Address) -> bool {
    let record = match get_service_registration(env, service) {
        Some(r) => r,
        None => return false,
    };

    let current_time = env.ledger().timestamp();
    current_time < record.registered_at.saturating_add(record.grace_period_seconds)
}

/// Evaluates penalty payout for an outage on a service.
/// Outages during grace period log event data without triggering penalty payouts (returns 0).
/// When grace period has expired or for unregistered services, returns the original penalty amount.
pub fn evaluate_outage_penalty_with_grace(
    env: &Env,
    service: &Address,
    outage_id: &Symbol,
    calculated_penalty: i128,
) -> i128 {
    if is_service_in_grace_period(env, service) {
        env.events().publish(
            (EVENT_GRACE_EXEMPT, EVENT_VERSION, service.clone()),
            (outage_id.clone(), calculated_penalty, 0i128),
        );
        0
    } else {
        calculated_penalty
    }
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

    // -------------------------------------------------------------------
    // Issue #672 Tests
    // -------------------------------------------------------------------

    #[test]
    fn test_service_registration_stored_in_instance_storage() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register_contract(None, crate::SLACalculatorContract);
        let admin = Address::generate(&env);
        let service = Address::generate(&env);

        env.ledger().set_timestamp(1_000_000);

        env.as_contract(&cid, || {
            setup(&env, &admin);
            assert_eq!(get_service_registration(&env, &service), None);

            let record = register_service(&env, &admin, &service).unwrap();
            assert_eq!(record.service, service);
            assert_eq!(record.registered_at, 1_000_000);
            assert_eq!(record.grace_period_seconds, 14 * 86_400);

            let stored = get_service_registration(&env, &service).unwrap();
            assert_eq!(stored.registered_at, 1_000_000);
            assert_eq!(stored.grace_period_seconds, 14 * 86_400);
        });
    }

    #[test]
    fn test_penalty_exemption_and_automatic_expiration_during_grace_period() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register_contract(None, crate::SLACalculatorContract);
        let admin = Address::generate(&env);
        let service = Address::generate(&env);
        let outage_id = symbol_short!("OUT1");

        let start_time = 1_000_000;
        env.ledger().set_timestamp(start_time);

        env.as_contract(&cid, || {
            setup(&env, &admin);
            register_service(&env, &admin, &service).unwrap();

            // During grace period (e.g. day 5: 5 * 86,400 seconds later)
            env.ledger().set_timestamp(start_time + 5 * 86_400);
            assert!(is_service_in_grace_period(&env, &service));

            // Standard penalty is 500, but in grace period it must evaluate to 0 (exempt)
            let penalty = evaluate_outage_penalty_with_grace(&env, &service, &outage_id, 500);
            assert_eq!(penalty, 0);

            // Exactly at expiration (14 days = 14 * 86,400 seconds)
            env.ledger().set_timestamp(start_time + 14 * 86_400);
            assert!(!is_service_in_grace_period(&env, &service));

            // Past grace period (e.g. day 15) -> full penalty applies
            env.ledger().set_timestamp(start_time + 15 * 86_400);
            assert!(!is_service_in_grace_period(&env, &service));

            let penalty_after = evaluate_outage_penalty_with_grace(&env, &service, &outage_id, 500);
            assert_eq!(penalty_after, 500);
        });
    }

    #[test]
    fn test_custom_grace_period_configuration() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register_contract(None, crate::SLACalculatorContract);
        let admin = Address::generate(&env);
        let stranger = Address::generate(&env);
        let service = Address::generate(&env);

        env.ledger().set_timestamp(1_000_000);

        env.as_contract(&cid, || {
            setup(&env, &admin);
            // Default is 14 days
            assert_eq!(get_grace_period_duration(&env), 14 * 86_400);

            // Stranger cannot set duration
            let err = set_grace_period_duration(&env, &stranger, 7 * 86_400);
            assert_eq!(err, Err(SLAError::Unauthorized));

            // Admin updates to 7 days
            set_grace_period_duration(&env, &admin, 7 * 86_400).unwrap();
            assert_eq!(get_grace_period_duration(&env), 7 * 86_400);

            let record = register_service(&env, &admin, &service).unwrap();
            assert_eq!(record.grace_period_seconds, 7 * 86_400);

            // Day 6: in grace period
            env.ledger().set_timestamp(1_000_000 + 6 * 86_400);
            assert!(is_service_in_grace_period(&env, &service));

            // Day 8: expired
            env.ledger().set_timestamp(1_000_000 + 8 * 86_400);
            assert!(!is_service_in_grace_period(&env, &service));
        });
    }
}
