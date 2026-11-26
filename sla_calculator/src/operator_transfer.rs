//! Two-step operator handoff helpers (#64, #1011, #1012).
//!
//! Storage layout matches the contract entrypoints in `lib.rs`: the pending
//! proposal is a `PendingProposalInfo` under `POP`, and the active operator is
//! the address under `OPERATOR`. A pending operator holds no operator rights
//! until it accepts; `require_active_operator` is the gate for that.
//!
//! There is only ever one proposal: a new `propose_operator` overwrites it, so
//! an address from a superseded proposal can no longer accept (#1012).

use soroban_sdk::{symbol_short, Address, Env, Symbol};

use crate::operator_query::{is_active_operator, live_pending_proposal};
use crate::PendingProposalInfo;

const ADMIN_KEY: Symbol = symbol_short!("ADMIN");
const OPERATOR_KEY: Symbol = symbol_short!("OPERATOR");
const PENDING_OP_KEY: Symbol = symbol_short!("POP");

#[derive(Debug, Clone, PartialEq)]
pub enum OperatorTransferError {
    /// The caller is not the current admin.
    NotAdmin,
    /// No proposal is stored.
    NoPendingProposal,
    /// The stored proposal is older than the expiration window.
    ProposalExpired,
    /// The caller is not the proposed operator.
    NotPendingOperator,
    /// The caller is not the active operator (this includes a pending operator
    /// that has not accepted yet).
    NotActiveOperator,
}

fn require_admin(env: &Env, admin: &Address) -> Result<(), OperatorTransferError> {
    let current: Option<Address> = env.storage().instance().get(&ADMIN_KEY);
    if current.as_ref() == Some(admin) {
        Ok(())
    } else {
        Err(OperatorTransferError::NotAdmin)
    }
}

/// Admin proposes a new operator; must be accepted before taking effect.
pub fn propose_operator(
    env: &Env,
    admin: &Address,
    new_operator: Address,
) -> Result<(), OperatorTransferError> {
    admin.require_auth();
    require_admin(env, admin)?;
    let proposal = PendingProposalInfo {
        target: new_operator,
        proposed_at: env.ledger().timestamp(),
    };
    env.storage().instance().set(&PENDING_OP_KEY, &proposal);
    Ok(())
}

/// The proposed operator accepts the role, replacing the current operator.
///
/// #1012: only the address in the *current* proposal can accept. A new
/// `propose_operator` overwrites the single stored proposal, so an address
/// from an earlier (superseded) proposal is rejected with
/// `NotPendingOperator` and leaves storage untouched: the caller is checked
/// before anything is written or removed, including the cleanup of an
/// expired proposal. A successful accept clears the proposal, so the same
/// proposal cannot be accepted twice.
pub fn accept_operator(env: &Env, new_operator: &Address) -> Result<(), OperatorTransferError> {
    new_operator.require_auth();
    let proposal: PendingProposalInfo = env
        .storage()
        .instance()
        .get(&PENDING_OP_KEY)
        .ok_or(OperatorTransferError::NoPendingProposal)?;
    if &proposal.target != new_operator {
        return Err(OperatorTransferError::NotPendingOperator);
    }
    if live_pending_proposal(env).is_none() {
        env.storage().instance().remove(&PENDING_OP_KEY);
        return Err(OperatorTransferError::ProposalExpired);
    }
    env.storage().instance().set(&OPERATOR_KEY, new_operator);
    env.storage().instance().remove(&PENDING_OP_KEY);
    Ok(())
}

/// Admin cancels a pending operator proposal before it is accepted.
pub fn cancel_operator_proposal(env: &Env, admin: &Address) -> Result<(), OperatorTransferError> {
    admin.require_auth();
    require_admin(env, admin)?;
    if !env.storage().instance().has(&PENDING_OP_KEY) {
        return Err(OperatorTransferError::NoPendingProposal);
    }
    env.storage().instance().remove(&PENDING_OP_KEY);
    Ok(())
}

/// Gate for operator-only actions: only the active operator passes. A pending
/// operator is rejected until it has accepted the role.
pub fn require_active_operator(env: &Env, caller: &Address) -> Result<(), OperatorTransferError> {
    if is_active_operator(env, caller) {
        Ok(())
    } else {
        Err(OperatorTransferError::NotActiveOperator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operator_query::{get_operator_state, PROPOSAL_EXPIRATION_SECONDS};
    use crate::SLACalculatorContract;
    use soroban_sdk::testutils::{Address as _, Ledger as _};

    struct Roles {
        admin: Address,
        operator: Address,
        successor: Address,
    }

    fn harness() -> (Env, Address, Roles) {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register_contract(None, SLACalculatorContract);
        let roles = Roles {
            admin: Address::generate(&env),
            operator: Address::generate(&env),
            successor: Address::generate(&env),
        };
        env.as_contract(&cid, || {
            env.storage().instance().set(&ADMIN_KEY, &roles.admin);
            env.storage().instance().set(&OPERATOR_KEY, &roles.operator);
        });
        (env, cid, roles)
    }

    #[test]
    fn pending_operator_is_rejected_by_gate_until_acceptance() {
        let (env, cid, roles) = harness();
        env.as_contract(&cid, || {
            propose_operator(&env, &roles.admin, roles.successor.clone()).unwrap();
            assert_eq!(
                require_active_operator(&env, &roles.successor),
                Err(OperatorTransferError::NotActiveOperator)
            );
            assert_eq!(require_active_operator(&env, &roles.operator), Ok(()));

            accept_operator(&env, &roles.successor).unwrap();
            assert_eq!(require_active_operator(&env, &roles.successor), Ok(()));
            assert_eq!(
                require_active_operator(&env, &roles.operator),
                Err(OperatorTransferError::NotActiveOperator)
            );
        });
    }

    #[test]
    fn only_the_proposed_operator_can_accept() {
        let (env, cid, roles) = harness();
        let stranger = Address::generate(&env);
        env.as_contract(&cid, || {
            propose_operator(&env, &roles.admin, roles.successor.clone()).unwrap();
            assert_eq!(
                accept_operator(&env, &stranger),
                Err(OperatorTransferError::NotPendingOperator)
            );
            let state = get_operator_state(&env);
            assert_eq!(state.current, Some(roles.operator.clone()));
            assert_eq!(state.pending, Some(roles.successor.clone()));
        });
    }

    #[test]
    fn only_the_admin_can_propose_or_cancel() {
        let (env, cid, roles) = harness();
        // Each `require_auth` call per address runs in its own frame.
        let run = |f: &dyn Fn()| env.as_contract(&cid, f);
        run(&|| {
            assert_eq!(
                propose_operator(&env, &roles.operator, roles.successor.clone()),
                Err(OperatorTransferError::NotAdmin)
            );
            assert_eq!(get_operator_state(&env).pending, None);
        });
        run(&|| propose_operator(&env, &roles.admin, roles.successor.clone()).unwrap());
        run(&|| {
            assert_eq!(
                cancel_operator_proposal(&env, &roles.operator),
                Err(OperatorTransferError::NotAdmin)
            );
        });
        run(&|| assert_eq!(cancel_operator_proposal(&env, &roles.admin), Ok(())));
        run(&|| {
            assert_eq!(
                cancel_operator_proposal(&env, &roles.admin),
                Err(OperatorTransferError::NoPendingProposal)
            );
            let state = get_operator_state(&env);
            assert_eq!(state.current, Some(roles.operator.clone()));
            assert_eq!(state.pending, None);
            assert_eq!(
                accept_operator(&env, &roles.successor),
                Err(OperatorTransferError::NoPendingProposal)
            );
        });
    }

    #[test]
    fn expired_proposal_cannot_be_accepted() {
        let (env, cid, roles) = harness();
        env.ledger().with_mut(|l| l.timestamp = 50);
        env.as_contract(&cid, || {
            propose_operator(&env, &roles.admin, roles.successor.clone()).unwrap();
        });
        env.ledger()
            .with_mut(|l| l.timestamp = 50 + PROPOSAL_EXPIRATION_SECONDS + 1);
        env.as_contract(&cid, || {
            assert_eq!(
                accept_operator(&env, &roles.successor),
                Err(OperatorTransferError::ProposalExpired)
            );
            assert!(!env.storage().instance().has(&PENDING_OP_KEY));
            assert_eq!(
                require_active_operator(&env, &roles.successor),
                Err(OperatorTransferError::NotActiveOperator)
            );
            assert_eq!(
                get_operator_state(&env).current,
                Some(roles.operator.clone())
            );
        });
    }

    fn stored_proposal(env: &Env) -> Option<PendingProposalInfo> {
        env.storage().instance().get(&PENDING_OP_KEY)
    }

    /// #1012: once a proposal is replaced, the earlier address is rejected
    /// and nothing in operator storage changes.
    #[test]
    fn superseded_operator_cannot_accept_and_state_is_unchanged() {
        let (env, cid, roles) = harness();
        let superseded = Address::generate(&env);
        let run = |f: &dyn Fn()| env.as_contract(&cid, f);

        env.ledger().with_mut(|l| l.timestamp = 10);
        run(&|| propose_operator(&env, &roles.admin, superseded.clone()).unwrap());
        env.ledger().with_mut(|l| l.timestamp = 20);
        run(&|| propose_operator(&env, &roles.admin, roles.successor.clone()).unwrap());

        run(&|| {
            let before = stored_proposal(&env);
            assert_eq!(
                before,
                Some(PendingProposalInfo {
                    target: roles.successor.clone(),
                    proposed_at: 20,
                }),
                "a new proposal must overwrite the previous one"
            );
            assert_eq!(
                accept_operator(&env, &superseded),
                Err(OperatorTransferError::NotPendingOperator)
            );
            assert_eq!(stored_proposal(&env), before);
            let state = get_operator_state(&env);
            assert_eq!(state.current, Some(roles.operator.clone()));
            assert_eq!(state.pending, Some(roles.successor.clone()));
            assert_eq!(
                require_active_operator(&env, &superseded),
                Err(OperatorTransferError::NotActiveOperator)
            );
        });
    }

    /// #1012: the current proposal is accepted once; any later accept, by it
    /// or by the superseded address, fails without changing the operator.
    #[test]
    fn current_proposal_accepts_exactly_once() {
        let (env, cid, roles) = harness();
        let superseded = Address::generate(&env);
        let run = |f: &dyn Fn()| env.as_contract(&cid, f);

        run(&|| propose_operator(&env, &roles.admin, superseded.clone()).unwrap());
        run(&|| propose_operator(&env, &roles.admin, roles.successor.clone()).unwrap());
        run(&|| {
            assert_eq!(
                accept_operator(&env, &superseded),
                Err(OperatorTransferError::NotPendingOperator)
            );
        });
        run(&|| assert_eq!(accept_operator(&env, &roles.successor), Ok(())));
        run(&|| {
            assert_eq!(stored_proposal(&env), None);
            assert_eq!(
                accept_operator(&env, &roles.successor),
                Err(OperatorTransferError::NoPendingProposal)
            );
        });
        run(&|| {
            assert_eq!(
                accept_operator(&env, &superseded),
                Err(OperatorTransferError::NoPendingProposal)
            );
            let state = get_operator_state(&env);
            assert_eq!(state.current, Some(roles.successor.clone()));
            assert_eq!(state.pending, None);
        });
    }

    /// #1012: a superseded address cannot even clear an expired proposal;
    /// only the proposed operator's own call drops it.
    #[test]
    fn superseded_operator_does_not_clear_an_expired_proposal() {
        let (env, cid, roles) = harness();
        let superseded = Address::generate(&env);
        let run = |f: &dyn Fn()| env.as_contract(&cid, f);

        env.ledger().with_mut(|l| l.timestamp = 100);
        run(&|| propose_operator(&env, &roles.admin, superseded.clone()).unwrap());
        run(&|| propose_operator(&env, &roles.admin, roles.successor.clone()).unwrap());
        env.ledger()
            .with_mut(|l| l.timestamp = 100 + PROPOSAL_EXPIRATION_SECONDS + 1);

        run(&|| {
            assert_eq!(
                accept_operator(&env, &superseded),
                Err(OperatorTransferError::NotPendingOperator)
            );
            assert!(env.storage().instance().has(&PENDING_OP_KEY));
        });
        run(&|| {
            assert_eq!(
                accept_operator(&env, &roles.successor),
                Err(OperatorTransferError::ProposalExpired)
            );
            assert!(!env.storage().instance().has(&PENDING_OP_KEY));
            assert_eq!(
                get_operator_state(&env).current,
                Some(roles.operator.clone())
            );
        });
    }

    /// #1012 through the contract entrypoints: the superseded address gets
    /// `Unauthorized`, the current one accepts once, then `NoPendingTransfer`.
    #[test]
    fn entrypoints_reject_superseded_operator_and_double_accept() {
        let env = Env::default();
        let cid = env.register_contract(None, SLACalculatorContract);
        let client = crate::SLACalculatorContractClient::new(&env, &cid);
        let admin = Address::generate(&env);
        let operator = Address::generate(&env);
        let superseded = Address::generate(&env);
        let latest = Address::generate(&env);
        client.initialize(&admin, &operator);

        client.propose_operator(&admin, &superseded);
        client.propose_operator(&admin, &latest);

        assert_eq!(
            client.try_accept_operator(&superseded),
            Err(Ok(crate::SLAError::Unauthorized))
        );
        assert_eq!(client.get_operator(), operator);
        let state = client.get_operator_state();
        assert_eq!(state.current, Some(operator.clone()));
        assert_eq!(state.pending, Some(latest.clone()));

        client.accept_operator(&latest);
        assert_eq!(client.get_operator(), latest);
        assert_eq!(
            client.try_accept_operator(&latest),
            Err(Ok(crate::SLAError::NoPendingTransfer))
        );
        assert_eq!(
            client.try_accept_operator(&superseded),
            Err(Ok(crate::SLAError::NoPendingTransfer))
        );
        assert_eq!(client.get_operator(), latest);
        assert_eq!(client.get_operator_state().pending, None);
    }
}
