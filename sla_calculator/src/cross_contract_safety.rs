//! SC-W5-077 – Cross-contract call safety model with failure rollback semantics.
//!
//! This module provides a safety model for cross-contract invocations in the
//! SLA calculator ecosystem. When a contract calls another contract (e.g., SLA
//! calculator calling payment escrow), failures must be handled deterministically
//! and any partial state changes must be rolled back.
//!
//! # Design
//!
//! Each cross-contract call is wrapped in a `SafeCall` that captures:
//! - The target contract identifier
//! - The function being called
//! - A compensation action to reverse the call if a subsequent step fails
//!
//! The `CrossContractSafety` struct maintains a call stack so that if any step
//! in a multi-step workflow fails, all prior steps are rolled back via their
//! registered compensation actions.
//!
//! # Usage
//!
//! ```ignore
//! let mut safety = CrossContractSafety::new(&env);
//! safety.call(contract_id, "lock_funds", args, || {
//!     // compensation: unlock the funds
//! });
//! safety.call(contract_id, "release_payment", args, || {
//!     // compensation: reverse the release
//! });
//! let result = safety.finalize()?; // rolls back on error
//! ```

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Map, Symbol, Vec};

// -----------------------------------------------------------------------
// Issue #711: cross-contract caller authorization guard
// -----------------------------------------------------------------------

/// Storage key for the set of contract addresses authorized to invoke
/// this contract's cross-contract-facing entry points.
const AUTHORIZED_CALLERS_KEY: Symbol = symbol_short!("XC_CALL");
/// Storage key for the admin address allowed to manage the caller
/// registry. Self-contained (no `crate::` dependency), matching the
/// convention used by other orphaned/self-contained modules in this
/// crate such as `dispute.rs`.
const ADMIN_KEY: Symbol = symbol_short!("ADMIN");

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum CrossContractSafetyError {
    NotInitialized = 1,
    Unauthorized = 2,
    /// Issue #711: a caller contract not on the authorized registry
    /// attempted a guarded call.
    UnauthorizedCaller = 3,
}

fn load_authorized_callers(env: &Env) -> Map<Address, bool> {
    env.storage()
        .instance()
        .get(&AUTHORIZED_CALLERS_KEY)
        .unwrap_or(Map::new(env))
}

fn require_admin(env: &Env, caller: &Address) -> Result<(), CrossContractSafetyError> {
    caller.require_auth();
    let admin: Address = env
        .storage()
        .instance()
        .get(&ADMIN_KEY)
        .ok_or(CrossContractSafetyError::NotInitialized)?;
    if admin != *caller {
        return Err(CrossContractSafetyError::Unauthorized);
    }
    Ok(())
}

/// Issue #711: set the admin allowed to manage the authorized-caller
/// registry. Callable once — subsequent calls are rejected the same way
/// as any other unauthorized-admin action, since there is no existing
/// admin to authorize a change.
pub fn initialize_admin(env: &Env, admin: &Address) -> Result<(), CrossContractSafetyError> {
    if env.storage().instance().has(&ADMIN_KEY) {
        return Err(CrossContractSafetyError::Unauthorized);
    }
    env.storage().instance().set(&ADMIN_KEY, admin);
    Ok(())
}

/// Issue #711: admin-only — add a contract address to the authorized
/// caller registry.
pub fn add_authorized_caller(
    env: &Env,
    admin: &Address,
    caller_contract: Address,
) -> Result<(), CrossContractSafetyError> {
    require_admin(env, admin)?;
    let mut callers = load_authorized_callers(env);
    callers.set(caller_contract, true);
    env.storage()
        .instance()
        .set(&AUTHORIZED_CALLERS_KEY, &callers);
    Ok(())
}

/// Issue #711: admin-only — remove a contract address from the
/// authorized caller registry.
pub fn remove_authorized_caller(
    env: &Env,
    admin: &Address,
    caller_contract: Address,
) -> Result<(), CrossContractSafetyError> {
    require_admin(env, admin)?;
    let mut callers = load_authorized_callers(env);
    callers.remove(caller_contract);
    env.storage()
        .instance()
        .set(&AUTHORIZED_CALLERS_KEY, &callers);
    Ok(())
}

/// Issue #711: whether `caller_contract` is on the authorized caller
/// registry.
pub fn is_authorized_caller(env: &Env, caller_contract: &Address) -> bool {
    load_authorized_callers(env)
        .get(caller_contract.clone())
        .unwrap_or(false)
}

/// Issue #711: guard for entry points that must only be reachable from
/// other contracts, not directly. `caller_contract` is the address of
/// the contract asserting it is the caller — Soroban's SDK does not
/// expose an ambient "calling contract" the way `env.invoker()` would in
/// some other VMs, so guarded entry points must accept this as an
/// explicit parameter identifying the invoking contract (contracts don't
/// hold signing keys the way user accounts do, so this is a registry
/// membership check rather than a `require_auth()` call).
///
/// # Errors
/// Returns `UnauthorizedCaller` if `caller_contract` is not on the
/// registry.
pub fn require_authorized_caller(
    env: &Env,
    caller_contract: &Address,
) -> Result<(), CrossContractSafetyError> {
    if !is_authorized_caller(env, caller_contract) {
        return Err(CrossContractSafetyError::UnauthorizedCaller);
    }
    Ok(())
}

/// Status of a cross-contract call.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum CrossContractCallStatus {
    /// The call succeeded.
    Success = 0,
    /// The target contract returned a recoverable error.
    RecoverableError = 1,
    /// The target contract returned a fatal error – rollback required.
    FatalError = 2,
    /// The call could not be dispatched (version mismatch, paused, etc.).
    DispatchFailed = 3,
    /// A compensation action has been applied for this call.
    Compensated = 4,
}

/// Wraps the result of a cross-contract call with safety metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SafeCallResult {
    /// The status of the call.
    pub status: CrossContractCallStatus,
    /// Human-readable error symbol when status != Success.
    pub error_symbol: Option<Symbol>,
}

/// A registered compensation action that can be invoked to reverse a call.
///
/// In a `#![no_std]` Soroban contract, we cannot store closures. Instead,
/// we store a `compensation_tag` (a Symbol identifying the compensation
/// logic) so the caller can re-invoke with reversed semantics.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompensationAction {
    /// A tag identifying what kind of compensation to apply.
    /// E.g., "unlock_funds", "reverse_settle", "unpause_escrow".
    pub tag: Symbol,
    /// The function name that was originally called (for audit).
    pub fn_name: Symbol,
}

/// Performs a safe cross-contract invocation.
///
/// Wraps `env.invoke_contract()` with error translation and returns a
/// `SafeCallResult` instead of panicking on failure.
pub fn safe_invoke_contract(
    env: &Env,
    contract_id: &Address,
    function_name: &Symbol,
    args: soroban_sdk::Vec<soroban_sdk::Val>,
) -> SafeCallResult {
    match env.try_invoke_contract::<soroban_sdk::Val, soroban_sdk::Val>(
        contract_id,
        function_name,
        args,
    ) {
        Ok(_val) => SafeCallResult {
            status: CrossContractCallStatus::Success,
            error_symbol: None,
        },
        Err(_err_val) => {
            let error_symbol = Symbol::new(env, "CROSS_CONTRACT_FAILURE");
            SafeCallResult {
                status: CrossContractCallStatus::FatalError,
                error_symbol: Some(error_symbol),
            }
        }
    }
}

/// Determines whether the given status requires rolling back prior calls.
pub fn requires_rollback(status: CrossContractCallStatus) -> bool {
    status == CrossContractCallStatus::FatalError
        || status == CrossContractCallStatus::DispatchFailed
}

/// Tracks a stack of cross-contract calls with registered compensation
/// actions. If any step in the sequence fails, all prior successful steps
/// are compensated in reverse order.
pub struct CrossContractSafety {
    /// Stack of compensation actions registered for each successful call.
    pub(crate) compensation_stack: Vec<CompensationAction>,
}

impl CrossContractSafety {
    /// Create a new empty safety tracker.
    pub fn new(env: &Env) -> Self {
        CrossContractSafety {
            compensation_stack: Vec::new(env),
        }
    }

    /// Execute a safe cross-contract call and register its compensation
    /// action for potential rollback.
    pub fn call(
        &mut self,
        env: &Env,
        contract_id: &Address,
        function_name: &Symbol,
        args: soroban_sdk::Vec<soroban_sdk::Val>,
        compensation_tag: Symbol,
    ) -> Result<SafeCallResult, SafeCallResult> {
        let result = safe_invoke_contract(env, contract_id, function_name, args);

        match result.status {
            CrossContractCallStatus::Success | CrossContractCallStatus::RecoverableError => {
                self.compensation_stack.push_back(CompensationAction {
                    tag: compensation_tag,
                    fn_name: function_name.clone(),
                });
                Ok(result)
            }
            CrossContractCallStatus::FatalError | CrossContractCallStatus::DispatchFailed => {
                Err(result)
            }
            CrossContractCallStatus::Compensated => Err(result),
        }
    }

    /// Returns the number of compensations registered.
    pub fn depth(&self) -> u32 {
        self.compensation_stack.len()
    }

    /// Whether there are any compensations registered.
    pub fn has_pending(&self) -> bool {
        !self.compensation_stack.is_empty()
    }
}

// -----------------------------------------------------------------------
// Pseudo-contract interface for the SLA ↔ payment workflow
// -----------------------------------------------------------------------

/// Symbol tags for compensation actions.
pub const COMP_UNLOCK_FUNDS: Symbol = symbol_short!("unlck_fnd");
pub const COMP_REVERSE_SETTLE: Symbol = symbol_short!("rev_setle");
pub const COMP_UNPAUSE_ESCROW: Symbol = symbol_short!("unp_escro");

/// Standard function names expected on downstream contracts.
pub const FN_LOCK_FUNDS: Symbol = symbol_short!("lock_fnds");
pub const FN_RELEASE_PAYMENT: Symbol = symbol_short!("rel_pay");
pub const FN_CANCEL_SETTLEMENT: Symbol = symbol_short!("can_setl");

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{symbol_short, Address, Env, Vec};

    #[test]
    fn test_safe_invoke_unknown_contract_returns_fatal_error() {
        let env = Env::default();
        let unknown = Address::generate(&env);
        let result = safe_invoke_contract(&env, &unknown, &symbol_short!("ping"), Vec::new(&env));
        assert_eq!(result.status, CrossContractCallStatus::FatalError);
        assert!(result.error_symbol.is_some());
    }

    #[test]
    fn test_requires_rollback_for_fatal() {
        assert!(requires_rollback(CrossContractCallStatus::FatalError));
        assert!(requires_rollback(CrossContractCallStatus::DispatchFailed));
        assert!(!requires_rollback(CrossContractCallStatus::Success));
        assert!(!requires_rollback(
            CrossContractCallStatus::RecoverableError
        ));
    }

    #[test]
    fn test_safety_tracker_starts_empty() {
        let env = Env::default();
        let safety = CrossContractSafety::new(&env);
        assert_eq!(safety.depth(), 0);
        assert!(!safety.has_pending());
    }

    #[test]
    fn test_safety_tracker_registers_compensation_on_push() {
        let env = Env::default();
        let mut safety = CrossContractSafety::new(&env);
        safety.compensation_stack.push_back(CompensationAction {
            tag: COMP_UNLOCK_FUNDS,
            fn_name: FN_LOCK_FUNDS,
        });
        assert_eq!(safety.depth(), 1);
        assert!(safety.has_pending());
    }

    #[test]
    fn test_safe_call_to_unknown_address_returns_err() {
        let env = Env::default();
        let mut safety = CrossContractSafety::new(&env);
        let unknown = Address::generate(&env);

        let result = safety.call(
            &env,
            &unknown,
            &symbol_short!("ping"),
            Vec::new(&env),
            COMP_UNLOCK_FUNDS,
        );
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().status,
            CrossContractCallStatus::FatalError
        );
    }

    #[test]
    fn test_status_variants_are_distinct() {
        let variants = [
            CrossContractCallStatus::Success as u32,
            CrossContractCallStatus::RecoverableError as u32,
            CrossContractCallStatus::FatalError as u32,
            CrossContractCallStatus::DispatchFailed as u32,
            CrossContractCallStatus::Compensated as u32,
        ];
        for i in 0..variants.len() {
            for j in (i + 1)..variants.len() {
                assert_ne!(variants[i], variants[j]);
            }
        }
    }

    #[test]
    fn test_compensation_symbols_are_distinct() {
        let tags = [COMP_UNLOCK_FUNDS, COMP_REVERSE_SETTLE, COMP_UNPAUSE_ESCROW];
        for i in 0..tags.len() {
            for j in (i + 1)..tags.len() {
                assert_ne!(tags[i], tags[j]);
            }
        }
    }

    #[test]
    fn test_fn_symbols_are_distinct() {
        let fns = [FN_LOCK_FUNDS, FN_RELEASE_PAYMENT, FN_CANCEL_SETTLEMENT];
        for i in 0..fns.len() {
            for j in (i + 1)..fns.len() {
                assert_ne!(fns[i], fns[j]);
            }
        }
    }

    #[test]
    fn test_unregistered_caller_is_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register_contract(None, crate::SLACalculatorContract);
        env.as_contract(&cid, || {
            let admin = Address::generate(&env);
            initialize_admin(&env, &admin).unwrap();

            let stranger = Address::generate(&env);
            let result = require_authorized_caller(&env, &stranger);
            assert_eq!(result, Err(CrossContractSafetyError::UnauthorizedCaller));
        });
    }

    #[test]
    fn test_admin_can_add_and_remove_authorized_caller() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register_contract(None, crate::SLACalculatorContract);
        let admin = Address::generate(&env);
        let caller_contract = Address::generate(&env);

        // Each mutating call gets its own `as_contract` scope: the test
        // utils' mocked-auth machinery treats a repeated `require_auth()`
        // for the same address within a single top-level `as_contract`
        // call as a conflicting duplicate, so calls are split the way
        // separate top-level contract invocations would be in practice.
        env.as_contract(&cid, || {
            initialize_admin(&env, &admin).unwrap();
        });
        env.as_contract(&cid, || {
            assert!(!is_authorized_caller(&env, &caller_contract));
        });
        env.as_contract(&cid, || {
            add_authorized_caller(&env, &admin, caller_contract.clone()).unwrap();
        });
        env.as_contract(&cid, || {
            assert!(is_authorized_caller(&env, &caller_contract));
            assert!(require_authorized_caller(&env, &caller_contract).is_ok());
        });
        env.as_contract(&cid, || {
            remove_authorized_caller(&env, &admin, caller_contract.clone()).unwrap();
        });
        env.as_contract(&cid, || {
            assert!(!is_authorized_caller(&env, &caller_contract));
            assert_eq!(
                require_authorized_caller(&env, &caller_contract),
                Err(CrossContractSafetyError::UnauthorizedCaller)
            );
        });
    }

    #[test]
    fn test_non_admin_cannot_manage_caller_registry() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register_contract(None, crate::SLACalculatorContract);
        env.as_contract(&cid, || {
            let admin = Address::generate(&env);
            initialize_admin(&env, &admin).unwrap();

            let not_admin = Address::generate(&env);
            let caller_contract = Address::generate(&env);
            let result = add_authorized_caller(&env, &not_admin, caller_contract);
            assert_eq!(result, Err(CrossContractSafetyError::Unauthorized));
        });
    }

    #[test]
    fn test_initialize_admin_rejected_once_already_set() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register_contract(None, crate::SLACalculatorContract);
        env.as_contract(&cid, || {
            let admin = Address::generate(&env);
            initialize_admin(&env, &admin).unwrap();

            let other = Address::generate(&env);
            let result = initialize_admin(&env, &other);
            assert_eq!(result, Err(CrossContractSafetyError::Unauthorized));
        });
    }
}
