// Emergency Admin DAO takeover protocol. Complements upgrade_path.rs.
use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Map, Symbol};

use crate::multi_operator;
use crate::ADMIN_KEY;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TakeoverError {
    AdminStillActive = 1,
    /// No takeover proposal is open (#1013).
    NoActiveProposal = 2,
    /// A takeover proposal is already open (#1013).
    ProposalAlreadyActive = 3,
    /// Caller is not in the current eligible voter (operator) set (#1013).
    NotEligibleVoter = 4,
    /// Approval weight from currently eligible voters is below the quorum
    /// recomputed at execution time (#1013).
    QuorumNotMet = 5,
}

const NINETY_DAYS_SECS: u64 = 90 * 24 * 60 * 60;
const TAKEOVER_EVENT: Symbol = symbol_short!("dao_take");
const TAKEOVER_PROPOSAL_KEY: Symbol = symbol_short!("DAOPROP");

/// An open DAO takeover proposal (#1013).
///
/// Quorum is deliberately *not* stored here: it is recomputed from the live
/// operator set when the proposal is executed. `membership_epoch` records the
/// operator-set epoch the proposal was opened under, for auditing.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TakeoverProposal {
    pub dao_contract: Address,
    pub proposed_at: u64,
    pub membership_epoch: u32,
    /// Approver -> membership epoch at which the approval was cast.
    pub approvals: Map<Address, u32>,
}

/// A registered DAO contract claims admin rights if the current admin
/// has been inactive for more than 90 days.
pub fn claim_admin_takeover(
    env: &Env,
    dao_contract: &Address,
    last_admin_activity: u64,
) -> Result<(), TakeoverError> {
    dao_contract.require_auth();
    if env.ledger().timestamp() < last_admin_activity + NINETY_DAYS_SECS {
        return Err(TakeoverError::AdminStillActive);
    }
    env.events()
        .publish((TAKEOVER_EVENT,), dao_contract.clone());
    Ok(())
}

fn require_eligible(env: &Env, voter: &Address) -> Result<(), TakeoverError> {
    voter.require_auth();
    multi_operator::is_operator(env, voter).map_err(|_| TakeoverError::NotEligibleVoter)
}

/// Returns the open takeover proposal, if any.
pub fn get_takeover_proposal(env: &Env) -> Option<TakeoverProposal> {
    env.storage().instance().get(&TAKEOVER_PROPOSAL_KEY)
}

/// Opens a takeover proposal in favour of `dao_contract`. The proposer must
/// be a current operator and its approval is recorded immediately.
pub fn propose_admin_takeover(
    env: &Env,
    proposer: &Address,
    dao_contract: &Address,
) -> Result<(), TakeoverError> {
    require_eligible(env, proposer)?;
    if get_takeover_proposal(env).is_some() {
        return Err(TakeoverError::ProposalAlreadyActive);
    }
    let epoch = multi_operator::membership_epoch(env);
    let mut approvals = Map::<Address, u32>::new(env);
    approvals.set(proposer.clone(), epoch);
    env.storage().instance().set(
        &TAKEOVER_PROPOSAL_KEY,
        &TakeoverProposal {
            dao_contract: dao_contract.clone(),
            proposed_at: env.ledger().timestamp(),
            membership_epoch: epoch,
            approvals,
        },
    );
    Ok(())
}

/// Records `approver`'s approval of the open takeover proposal. The approver
/// must be a current operator. Re-approving refreshes the recorded epoch.
pub fn approve_admin_takeover(env: &Env, approver: &Address) -> Result<(), TakeoverError> {
    require_eligible(env, approver)?;
    let mut proposal = get_takeover_proposal(env).ok_or(TakeoverError::NoActiveProposal)?;
    proposal
        .approvals
        .set(approver.clone(), multi_operator::membership_epoch(env));
    env.storage()
        .instance()
        .set(&TAKEOVER_PROPOSAL_KEY, &proposal);
    Ok(())
}

/// Quorum for the *current* eligible voter set: a strict majority of the
/// active operators, recomputed from live membership on every call.
pub fn current_takeover_quorum(env: &Env) -> u32 {
    let eligible = multi_operator::list_operators(env)
        .map(|ops| ops.len())
        .unwrap_or(0);
    eligible / 2 + 1
}

/// Approval weight of `proposal` counted against the *current* voter set.
///
/// An approval only counts if its approver is an operator now and has been
/// one continuously since the approval was cast (i.e. it has not been removed
/// and re-added since). Weight from removed members is therefore dropped.
pub fn current_approval_weight(env: &Env, proposal: &TakeoverProposal) -> u32 {
    let mut weight = 0u32;
    for (approver, approved_at) in proposal.approvals.iter() {
        if let Some(joined) = multi_operator::joined_epoch(env, &approver) {
            if joined <= approved_at {
                weight = weight.saturating_add(1);
            }
        }
    }
    weight
}

/// Executes the open takeover proposal, transferring admin to the DAO.
///
/// The eligible voter set and quorum are revalidated at execution: a count
/// snapshotted at proposal time is never trusted. If the fresh approval
/// weight is below the fresh quorum the call fails with `QuorumNotMet` and
/// no state is changed.
pub fn execute_admin_takeover(
    env: &Env,
    executor: &Address,
    last_admin_activity: u64,
) -> Result<(), TakeoverError> {
    require_eligible(env, executor)?;
    if env.ledger().timestamp() < last_admin_activity.saturating_add(NINETY_DAYS_SECS) {
        return Err(TakeoverError::AdminStillActive);
    }
    let proposal = get_takeover_proposal(env).ok_or(TakeoverError::NoActiveProposal)?;

    let quorum = current_takeover_quorum(env);
    let weight = current_approval_weight(env, &proposal);
    if weight < quorum {
        return Err(TakeoverError::QuorumNotMet);
    }

    env.storage()
        .instance()
        .set(&ADMIN_KEY, &proposal.dao_contract);
    env.storage().instance().remove(&TAKEOVER_PROPOSAL_KEY);
    env.events().publish(
        (TAKEOVER_EVENT,),
        (
            proposal.dao_contract.clone(),
            weight,
            quorum,
            multi_operator::membership_epoch(env),
        ),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SLACalculatorContract;
    use soroban_sdk::testutils::{Address as _, Ledger};

    const LAST_ACTIVITY: u64 = 1_000;

    struct Setup {
        env: Env,
        cid: Address,
        admin: Address,
        dao: Address,
        ops: [Address; 3],
    }

    impl Setup {
        /// Runs `f` as its own contract invocation, so every `require_auth`
        /// is checked in a fresh frame (as separate transactions would be).
        fn call<T>(&self, f: impl FnOnce() -> T) -> T {
            self.env.as_contract(&self.cid, f)
        }

        fn admin_of_contract(&self) -> Address {
            self.call(|| self.env.storage().instance().get(&ADMIN_KEY).unwrap())
        }
    }

    /// Admin plus three operators (quorum 2 of 3), with the admin inactive.
    fn setup() -> Setup {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger()
            .with_mut(|l| l.timestamp = LAST_ACTIVITY + NINETY_DAYS_SECS + 1);
        let cid = env.register_contract(None, SLACalculatorContract);
        let admin = Address::generate(&env);
        let dao = Address::generate(&env);
        let ops = [
            Address::generate(&env),
            Address::generate(&env),
            Address::generate(&env),
        ];
        let s = Setup {
            env,
            cid,
            admin,
            dao,
            ops,
        };
        s.call(|| {
            s.env.storage().instance().set(&ADMIN_KEY, &s.admin);
            multi_operator::init_operators(&s.env, &s.ops[0]);
            multi_operator::add_operator(&s.env, &s.admin, &s.ops[1]).unwrap();
            multi_operator::add_operator(&s.env, &s.admin, &s.ops[2]).unwrap();
        });
        s
    }

    fn propose_and_approve(s: &Setup) {
        s.call(|| propose_admin_takeover(&s.env, &s.ops[0], &s.dao))
            .unwrap();
        s.call(|| approve_admin_takeover(&s.env, &s.ops[1]))
            .unwrap();
    }

    fn execute(s: &Setup) -> Result<(), TakeoverError> {
        s.call(|| execute_admin_takeover(&s.env, &s.ops[0], LAST_ACTIVITY))
    }

    #[test]
    fn executes_when_membership_unchanged_and_quorum_met() {
        let s = setup();
        propose_and_approve(&s);
        assert_eq!(s.call(|| current_takeover_quorum(&s.env)), 2);
        execute(&s).unwrap();
        assert!(s.call(|| get_takeover_proposal(&s.env)).is_none());
        assert_eq!(s.admin_of_contract(), s.dao);
    }

    #[test]
    fn removed_approver_weight_is_dropped_and_execution_fails() {
        let s = setup();
        propose_and_approve(&s);
        // Membership change between proposal and execution.
        s.call(|| multi_operator::remove_operator(&s.env, &s.admin, &s.ops[1]))
            .unwrap();
        let before = s.call(|| get_takeover_proposal(&s.env)).unwrap();
        // Two operators remain -> quorum 2, but only one approval is still eligible.
        assert_eq!(s.call(|| current_takeover_quorum(&s.env)), 2);
        assert_eq!(s.call(|| current_approval_weight(&s.env, &before)), 1);
        assert_eq!(execute(&s), Err(TakeoverError::QuorumNotMet));
        // No state change on failure.
        assert_eq!(s.admin_of_contract(), s.admin);
        assert_eq!(s.call(|| get_takeover_proposal(&s.env)), Some(before));
    }

    #[test]
    fn removed_then_readded_approver_must_approve_again() {
        let s = setup();
        propose_and_approve(&s);
        s.call(|| multi_operator::remove_operator(&s.env, &s.admin, &s.ops[1]))
            .unwrap();
        s.call(|| multi_operator::add_operator(&s.env, &s.admin, &s.ops[1]))
            .unwrap();
        assert_eq!(execute(&s), Err(TakeoverError::QuorumNotMet));
        s.call(|| approve_admin_takeover(&s.env, &s.ops[1]))
            .unwrap();
        execute(&s).unwrap();
        assert_eq!(s.admin_of_contract(), s.dao);
    }

    #[test]
    fn membership_addition_raising_quorum_fails_execution() {
        let s = setup();
        propose_and_approve(&s);
        let before = s.call(|| get_takeover_proposal(&s.env)).unwrap();
        for _ in 0..2 {
            let newcomer = Address::generate(&s.env);
            s.call(|| multi_operator::add_operator(&s.env, &s.admin, &newcomer))
                .unwrap();
        }
        assert!(s.call(|| multi_operator::membership_epoch(&s.env)) > before.membership_epoch);
        // Five operators now -> quorum 3; the two earlier approvals no longer suffice.
        assert_eq!(s.call(|| current_takeover_quorum(&s.env)), 3);
        assert_eq!(execute(&s), Err(TakeoverError::QuorumNotMet));
        assert_eq!(s.admin_of_contract(), s.admin);
        assert_eq!(s.call(|| get_takeover_proposal(&s.env)), Some(before));
    }

    #[test]
    fn non_operator_cannot_propose_approve_or_execute() {
        let s = setup();
        let outsider = Address::generate(&s.env);
        assert_eq!(
            s.call(|| propose_admin_takeover(&s.env, &outsider, &s.dao)),
            Err(TakeoverError::NotEligibleVoter)
        );
        s.call(|| propose_admin_takeover(&s.env, &s.ops[0], &s.dao))
            .unwrap();
        assert_eq!(
            s.call(|| approve_admin_takeover(&s.env, &outsider)),
            Err(TakeoverError::NotEligibleVoter)
        );
        assert_eq!(
            s.call(|| execute_admin_takeover(&s.env, &outsider, LAST_ACTIVITY)),
            Err(TakeoverError::NotEligibleVoter)
        );
    }
}
