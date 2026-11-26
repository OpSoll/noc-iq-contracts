use soroban_sdk::{symbol_short, Address, Env, Symbol};

use crate::admin_query::get_admin_state;

const PENDING_ADMIN_KEY: Symbol = symbol_short!("PADMIN");
const PENDING_ADMIN_TS_KEY: Symbol = symbol_short!("PADMINTS");
const ADMIN_KEY: Symbol = symbol_short!("ADMIN");
const ADMIN_PROPOSED_EVENT: Symbol = symbol_short!("adm_prop");
const ADMIN_ACCEPTED_EVENT: Symbol = symbol_short!("adm_acpt");
const PROPOSAL_EXPIRATION_SECONDS: u64 = 604_800;

#[derive(Debug, Clone, PartialEq)]
pub enum AdminTransferError {
    ProposalExpired,
    NoPendingProposal,
    NoCurrentAdmin,
    NotPendingAdmin,
    AdminRenounced,
}

/// Current admin proposes a new admin address.
pub fn propose_admin(env: &Env, admin: &Address, new_admin: Address) {
    admin.require_auth();
    let state = get_admin_state(env);
    assert!(!state.renounced, "admin role has been renounced");
    assert!(
        state.current.as_ref() == Some(admin),
        "only the current admin may propose a successor"
    );
    env.storage().instance().set(&PENDING_ADMIN_KEY, &new_admin);
    env.storage()
        .instance()
        .set(&PENDING_ADMIN_TS_KEY, &env.ledger().timestamp());
    env.events().publish((ADMIN_PROPOSED_EVENT,), new_admin);
}

/// The proposed admin accepts the role within the expiration window.
///
/// #1012: only the address in the *current* proposal can accept. A new
/// `propose_admin` overwrites the stored proposal, so an address from a
/// superseded proposal is rejected with `NotPendingAdmin` before any storage
/// is written or removed (including the cleanup of an expired proposal). A
/// successful accept clears the proposal, so it cannot be accepted twice.
pub fn accept_admin(env: &Env, new_admin: &Address) -> Result<(), AdminTransferError> {
    new_admin.require_auth();
    let state = get_admin_state(env);
    if state.renounced {
        return Err(AdminTransferError::AdminRenounced);
    }
    let proposed_at: u64 = env
        .storage()
        .instance()
        .get(&PENDING_ADMIN_TS_KEY)
        .ok_or(AdminTransferError::NoPendingProposal)?;
    if state.pending.as_ref() != Some(new_admin) {
        return Err(AdminTransferError::NotPendingAdmin);
    }
    if env.ledger().timestamp() > proposed_at.saturating_add(PROPOSAL_EXPIRATION_SECONDS) {
        env.storage().instance().remove(&PENDING_ADMIN_KEY);
        env.storage().instance().remove(&PENDING_ADMIN_TS_KEY);
        return Err(AdminTransferError::ProposalExpired);
    }
    let old_admin = state
        .current
        .ok_or(AdminTransferError::NoCurrentAdmin)?;
    env.storage().instance().set(&ADMIN_KEY, new_admin);
    env.storage().instance().remove(&PENDING_ADMIN_KEY);
    env.storage().instance().remove(&PENDING_ADMIN_TS_KEY);
    env.events()
        .publish((ADMIN_ACCEPTED_EVENT,), (old_admin, new_admin.clone()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SLACalculatorContract, ADMIN_KEY};
    use soroban_sdk::testutils::{Address as _, Ledger as _};

    #[test]
    fn acceptance_requires_the_latest_pending_address() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register_contract(None, SLACalculatorContract);
        let admin = Address::generate(&env);
        let superseded = Address::generate(&env);
        let latest = Address::generate(&env);

        // Each `require_auth` call per address runs in its own frame.
        env.as_contract(&cid, || {
            env.storage().instance().set(&ADMIN_KEY, &admin);
            propose_admin(&env, &admin, superseded.clone());
        });
        env.as_contract(&cid, || {
            propose_admin(&env, &admin, latest.clone());

            assert_eq!(
                accept_admin(&env, &superseded),
                Err(AdminTransferError::NotPendingAdmin)
            );
            assert_eq!(accept_admin(&env, &latest), Ok(()));

            let stored: Address = env.storage().instance().get(&ADMIN_KEY).unwrap();
            assert_eq!(stored, latest);
            let pending: Option<Address> = env.storage().instance().get(&PENDING_ADMIN_KEY);
            assert_eq!(pending, None);
        });
    }

    #[test]
    fn acceptance_without_a_proposal_is_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register_contract(None, SLACalculatorContract);
        let admin = Address::generate(&env);
        let stranger = Address::generate(&env);

        env.as_contract(&cid, || {
            env.storage().instance().set(&ADMIN_KEY, &admin);
            assert_eq!(
                accept_admin(&env, &stranger),
                Err(AdminTransferError::NoPendingProposal)
            );
            let stored: Address = env.storage().instance().get(&ADMIN_KEY).unwrap();
            assert_eq!(stored, admin);
        });
    }

    /// #1012: a superseded address is rejected with zero state change, the
    /// current proposal is accepted exactly once, and nothing can accept it
    /// again afterwards.
    #[test]
    fn superseded_admin_is_rejected_and_current_proposal_accepts_once() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register_contract(None, SLACalculatorContract);
        let admin = Address::generate(&env);
        let superseded = Address::generate(&env);
        let latest = Address::generate(&env);
        let run = |f: &dyn Fn()| env.as_contract(&cid, f);

        run(&|| {
            env.storage().instance().set(&ADMIN_KEY, &admin);
            propose_admin(&env, &admin, superseded.clone());
        });
        run(&|| propose_admin(&env, &admin, latest.clone()));

        run(&|| {
            let ts_before: u64 = env.storage().instance().get(&PENDING_ADMIN_TS_KEY).unwrap();
            assert_eq!(
                accept_admin(&env, &superseded),
                Err(AdminTransferError::NotPendingAdmin)
            );
            let state = get_admin_state(&env);
            assert_eq!(state.current, Some(admin.clone()));
            assert_eq!(state.pending, Some(latest.clone()));
            let ts_after: u64 = env.storage().instance().get(&PENDING_ADMIN_TS_KEY).unwrap();
            assert_eq!(ts_after, ts_before);
        });

        run(&|| assert_eq!(accept_admin(&env, &latest), Ok(())));
        run(&|| {
            assert_eq!(
                accept_admin(&env, &latest),
                Err(AdminTransferError::NoPendingProposal)
            );
        });
        run(&|| {
            assert_eq!(
                accept_admin(&env, &superseded),
                Err(AdminTransferError::NoPendingProposal)
            );
            let state = get_admin_state(&env);
            assert_eq!(state.current, Some(latest.clone()));
            assert_eq!(state.pending, None);
        });
    }

    /// #1012: a superseded address cannot clear an expired proposal either;
    /// only the proposed admin's own call drops it.
    #[test]
    fn superseded_admin_does_not_clear_an_expired_proposal() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register_contract(None, SLACalculatorContract);
        let admin = Address::generate(&env);
        let superseded = Address::generate(&env);
        let latest = Address::generate(&env);
        let run = |f: &dyn Fn()| env.as_contract(&cid, f);

        env.ledger().with_mut(|l| l.timestamp = 100);
        run(&|| {
            env.storage().instance().set(&ADMIN_KEY, &admin);
            propose_admin(&env, &admin, superseded.clone());
        });
        run(&|| propose_admin(&env, &admin, latest.clone()));
        env.ledger()
            .with_mut(|l| l.timestamp = 100 + PROPOSAL_EXPIRATION_SECONDS + 1);

        run(&|| {
            assert_eq!(
                accept_admin(&env, &superseded),
                Err(AdminTransferError::NotPendingAdmin)
            );
            assert_eq!(get_admin_state(&env).pending, Some(latest.clone()));
            assert!(env.storage().instance().has(&PENDING_ADMIN_TS_KEY));
        });
        run(&|| {
            assert_eq!(
                accept_admin(&env, &latest),
                Err(AdminTransferError::ProposalExpired)
            );
            let state = get_admin_state(&env);
            assert_eq!(state.current, Some(admin.clone()));
            assert_eq!(state.pending, None);
        });
    }
}
