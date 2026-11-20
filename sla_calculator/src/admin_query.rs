use soroban_sdk::{symbol_short, Address, Env, Symbol};

const ADMIN_KEY: Symbol = symbol_short!("ADMIN");
const PENDING_ADMIN_KEY: Symbol = symbol_short!("PADMIN");
pub(crate) const ADMIN_RENOUNCED_KEY: Symbol = symbol_short!("ADMREN");

#[derive(Debug, Clone, PartialEq)]
pub enum QueryError {
    NotInitialized,
    Renounced,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AdminState {
    pub current: Option<Address>,
    pub pending: Option<Address>,
    pub renounced: bool,
}

/// Returns the current admin address, or `NotInitialized` if unset.
pub fn get_admin(env: &Env) -> Result<Address, QueryError> {
    if env.storage().instance().has(&ADMIN_RENOUNCED_KEY) {
        return Err(QueryError::Renounced);
    }
    env.storage()
        .instance()
        .get(&ADMIN_KEY)
        .ok_or(QueryError::NotInitialized)
}

pub fn get_admin_state(env: &Env) -> AdminState {
    AdminState {
        current: env.storage().instance().get(&ADMIN_KEY),
        pending: env.storage().instance().get(&PENDING_ADMIN_KEY),
        renounced: env.storage().instance().has(&ADMIN_RENOUNCED_KEY),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::admin_renounce::renounce_admin;
    use crate::admin_transfer::{accept_admin, propose_admin};
    use crate::{SLACalculatorContract, ADMIN_KEY};
    use soroban_sdk::testutils::{Address as _, Events as _};

    fn harness() -> (Env, soroban_sdk::Address) {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register_contract(None, SLACalculatorContract);
        (env, cid)
    }

    #[test]
    fn admin_state_tracks_initialized_pending_accepted_and_renounced() {
        let (env, cid) = harness();
        let admin = Address::generate(&env);
        let pending = Address::generate(&env);
        env.as_contract(&cid, || {
            env.storage().instance().set(&ADMIN_KEY, &admin);

            let initialized = get_admin_state(&env);
            assert_eq!(initialized.current, Some(admin.clone()));
            assert_eq!(initialized.pending, None);
            assert!(!initialized.renounced);
            assert_eq!(get_admin(&env), Ok(admin.clone()));

            propose_admin(&env, &admin, pending.clone());
            let proposed = get_admin_state(&env);
            assert_eq!(proposed.current, Some(admin.clone()));
            assert_eq!(proposed.pending, Some(pending.clone()));
            assert!(!proposed.renounced);

            assert_eq!(accept_admin(&env, &pending), Ok(()));
            let accepted = get_admin_state(&env);
            assert_eq!(accepted.current, Some(pending.clone()));
            assert_eq!(accepted.pending, None);
            assert!(!accepted.renounced);
            assert_eq!(get_admin(&env), Ok(pending.clone()));

            renounce_admin(&env, &pending);
            let renounced = get_admin_state(&env);
            assert_eq!(renounced.current, None);
            assert_eq!(renounced.pending, None);
            assert!(renounced.renounced);
            assert_eq!(get_admin(&env), Err(QueryError::Renounced));
        });
    }

    #[test]
    fn admin_query_is_read_only_and_stable_across_calls() {
        let (env, cid) = harness();
        let admin = Address::generate(&env);
        let pending = Address::generate(&env);
        env.as_contract(&cid, || {
            env.storage().instance().set(&ADMIN_KEY, &admin);
            propose_admin(&env, &admin, pending.clone());

            let events_before = env.events().all().len();
            let first = get_admin_state(&env);
            let second = get_admin_state(&env);
            let third = get_admin_state(&env);

            assert_eq!(first, second);
            assert_eq!(second, third);
            assert_eq!(first.pending, Some(pending));
            assert_eq!(env.events().all().len(), events_before);
        });
    }
}
