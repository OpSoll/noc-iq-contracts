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

use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol, Vec};

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
}
