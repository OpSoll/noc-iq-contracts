//! #1011 – Operator role queries.
//!
//! Off-chain tooling needs an unambiguous answer to "who can act as the
//! operator right now?" while a two-step operator handoff (#64) is in flight.
//! The active operator and the pending (proposed, not yet accepted) operator
//! are therefore reported as separate fields: a pending operator is never
//! reported as `current`, and holds no operator rights until it calls
//! `accept_operator`.

use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol};

use crate::PendingProposalInfo;

const OPERATOR_KEY: Symbol = symbol_short!("OPERATOR");
const PENDING_OP_KEY: Symbol = symbol_short!("POP");
/// Mirrors the contract's proposal lifetime (7 days).
pub(crate) const PROPOSAL_EXPIRATION_SECONDS: u64 = 604_800;

#[derive(Debug, Clone, PartialEq)]
pub enum QueryError {
    NotInitialized,
}

/// Operator role snapshot returned by `get_operator_state`.
///
/// - `current` is the only address that may call operator-gated entrypoints.
/// - `pending` is a proposed operator that has not accepted yet; it has no
///   operator rights. Expired proposals are reported as `None`.
/// - `pending_expires_at` is the ledger timestamp (seconds) after which the
///   pending proposal can no longer be accepted; `None` when nothing is pending.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperatorRoleState {
    pub current: Option<Address>,
    pub pending: Option<Address>,
    pub pending_expires_at: Option<u64>,
}

/// Returns the current operator address, or `NotInitialized` if unset.
pub fn get_operator(env: &Env) -> Result<Address, QueryError> {
    env.storage()
        .instance()
        .get(&OPERATOR_KEY)
        .ok_or(QueryError::NotInitialized)
}

/// Returns the pending proposal if one exists and has not expired.
///
/// Read-only: an expired proposal is reported as absent but is not removed
/// from storage here.
pub(crate) fn live_pending_proposal(env: &Env) -> Option<PendingProposalInfo> {
    let proposal: PendingProposalInfo = env.storage().instance().get(&PENDING_OP_KEY)?;
    let age = env
        .ledger()
        .timestamp()
        .saturating_sub(proposal.proposed_at);
    if age <= PROPOSAL_EXPIRATION_SECONDS {
        Some(proposal)
    } else {
        None
    }
}

/// Returns the pending (not yet accepted) operator, if any.
pub fn get_pending_operator(env: &Env) -> Option<Address> {
    live_pending_proposal(env).map(|p| p.target)
}

/// Returns the active and pending operator identities as separate fields.
pub fn get_operator_state(env: &Env) -> OperatorRoleState {
    let pending = live_pending_proposal(env);
    OperatorRoleState {
        current: env.storage().instance().get(&OPERATOR_KEY),
        pending_expires_at: pending
            .as_ref()
            .map(|p| p.proposed_at.saturating_add(PROPOSAL_EXPIRATION_SECONDS)),
        pending: pending.map(|p| p.target),
    }
}

/// True only for the active operator. A pending operator is not active.
pub fn is_active_operator(env: &Env, caller: &Address) -> bool {
    matches!(get_operator(env), Ok(operator) if &operator == caller)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SLACalculatorContract, SLACalculatorContractClient, SLAError};
    use soroban_sdk::testutils::{Address as _, Events as _, Ledger as _};

    struct Roles {
        admin: Address,
        operator: Address,
        successor: Address,
    }

    fn setup() -> (Env, SLACalculatorContractClient<'static>, Roles) {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register_contract(None, SLACalculatorContract);
        let client = SLACalculatorContractClient::new(&env, &cid);
        let roles = Roles {
            admin: Address::generate(&env),
            operator: Address::generate(&env),
            successor: Address::generate(&env),
        };
        client.initialize(&roles.admin, &roles.operator);
        (env, client, roles)
    }

    fn calc(
        env: &Env,
        client: &SLACalculatorContractClient<'static>,
        caller: &Address,
        outage: &str,
    ) -> Result<(), SLAError> {
        match client.try_calculate_sla(
            caller,
            &Symbol::new(env, outage),
            &symbol_short!("critical"),
            &10,
        ) {
            Ok(_) => Ok(()),
            Err(Ok(err)) => Err(err),
            Err(Err(_)) => panic!("unexpected host error"),
        }
    }

    #[test]
    fn state_without_pending_transfer_reports_only_current() {
        let (_env, client, roles) = setup();
        let state = client.get_operator_state();
        assert_eq!(state.current, Some(roles.operator.clone()));
        assert_eq!(state.pending, None);
        assert_eq!(state.pending_expires_at, None);
    }

    #[test]
    fn state_with_pending_transfer_reports_current_and_pending_separately() {
        let (env, client, roles) = setup();
        env.ledger().with_mut(|l| l.timestamp = 1_000);
        client.propose_operator(&roles.admin, &roles.successor);

        let state = client.get_operator_state();
        assert_eq!(state.current, Some(roles.operator.clone()));
        assert_eq!(state.pending, Some(roles.successor.clone()));
        assert_eq!(
            state.pending_expires_at,
            Some(1_000 + PROPOSAL_EXPIRATION_SECONDS)
        );
        // The legacy single-value getter still reports only the active operator.
        assert_eq!(client.get_operator(), roles.operator);
    }

    #[test]
    fn state_after_acceptance_promotes_pending_to_current() {
        let (_env, client, roles) = setup();
        client.propose_operator(&roles.admin, &roles.successor);
        client.accept_operator(&roles.successor);

        let state = client.get_operator_state();
        assert_eq!(state.current, Some(roles.successor.clone()));
        assert_eq!(state.pending, None);
        assert_eq!(state.pending_expires_at, None);
    }

    #[test]
    fn state_after_cancellation_keeps_current_and_clears_pending() {
        let (_env, client, roles) = setup();
        client.propose_operator(&roles.admin, &roles.successor);
        client.cancel_operator_proposal(&roles.admin);

        let state = client.get_operator_state();
        assert_eq!(state.current, Some(roles.operator.clone()));
        assert_eq!(state.pending, None);
        assert_eq!(state.pending_expires_at, None);
    }

    #[test]
    fn expired_proposal_is_not_reported_and_query_does_not_mutate_storage() {
        let (env, client, roles) = setup();
        env.ledger().with_mut(|l| l.timestamp = 1_000);
        client.propose_operator(&roles.admin, &roles.successor);
        env.ledger()
            .with_mut(|l| l.timestamp = 1_000 + PROPOSAL_EXPIRATION_SECONDS + 1);

        let events_before = env.events().all().len();
        let state = client.get_operator_state();
        assert_eq!(state.current, Some(roles.operator.clone()));
        assert_eq!(state.pending, None);
        assert_eq!(state.pending_expires_at, None);
        assert_eq!(client.get_operator_state(), state);
        assert_eq!(env.events().all().len(), events_before);
        env.as_contract(&client.address, || {
            assert!(env.storage().instance().has(&PENDING_OP_KEY));
        });
    }

    #[test]
    fn pending_operator_cannot_invoke_operator_gated_entrypoints_before_acceptance() {
        let (env, client, roles) = setup();
        client.propose_operator(&roles.admin, &roles.successor);

        assert_eq!(
            calc(&env, &client, &roles.successor, "OUT_PEND"),
            Err(SLAError::Unauthorized)
        );
        assert_eq!(
            client.try_renounce_operator(&roles.successor),
            Err(Ok(SLAError::Unauthorized))
        );
        // The active operator keeps working while the transfer is pending.
        assert_eq!(calc(&env, &client, &roles.operator, "OUT_CUR"), Ok(()));

        let state = client.get_operator_state();
        assert_eq!(state.current, Some(roles.operator.clone()));
        assert_eq!(state.pending, Some(roles.successor.clone()));
    }

    #[test]
    fn pending_operator_gains_rights_only_after_acceptance() {
        let (env, client, roles) = setup();
        client.propose_operator(&roles.admin, &roles.successor);
        client.accept_operator(&roles.successor);

        assert_eq!(calc(&env, &client, &roles.successor, "OUT_NEW"), Ok(()));
        assert_eq!(
            calc(&env, &client, &roles.operator, "OUT_OLD"),
            Err(SLAError::Unauthorized)
        );
    }

    #[test]
    fn cancelled_pending_operator_never_gains_rights() {
        let (env, client, roles) = setup();
        client.propose_operator(&roles.admin, &roles.successor);
        client.cancel_operator_proposal(&roles.admin);

        assert_eq!(
            calc(&env, &client, &roles.successor, "OUT_CAN"),
            Err(SLAError::Unauthorized)
        );
        assert_eq!(
            client.try_accept_operator(&roles.successor),
            Err(Ok(SLAError::NoPendingTransfer))
        );
    }

    #[test]
    fn helper_queries_distinguish_active_from_pending() {
        let (env, client, roles) = setup();
        client.propose_operator(&roles.admin, &roles.successor);
        env.as_contract(&client.address, || {
            assert_eq!(get_operator(&env), Ok(roles.operator.clone()));
            assert_eq!(get_pending_operator(&env), Some(roles.successor.clone()));
            assert!(is_active_operator(&env, &roles.operator));
            assert!(!is_active_operator(&env, &roles.successor));
        });
    }
}
