use soroban_sdk::{symbol_short, Address, Env, Symbol};

use crate::admin_query::{get_admin_state, ADMIN_RENOUNCED_KEY};

const ADMIN_KEY: Symbol = symbol_short!("ADMIN");
const PENDING_ADMIN_KEY: Symbol = symbol_short!("PADMIN");
const PENDING_ADMIN_TS_KEY: Symbol = symbol_short!("PADMINTS");
const ADMIN_RENOUNCED_EVENT: Symbol = symbol_short!("adm_renc");

/// Permanently renounces the admin role, locking admin-gated methods.
/// Caller must be the current admin; authentication is enforced here.
pub fn renounce_admin(env: &Env, caller: &Address) {
    caller.require_auth();
    let state = get_admin_state(env);
    assert!(!state.renounced, "admin role has already been renounced");
    assert!(
        state.current.as_ref() == Some(caller),
        "only the current admin may renounce"
    );
    env.storage().instance().remove(&ADMIN_KEY);
    env.storage().instance().remove(&PENDING_ADMIN_KEY);
    env.storage().instance().remove(&PENDING_ADMIN_TS_KEY);
    env.storage().instance().set(&ADMIN_RENOUNCED_KEY, &true);
    env.events()
        .publish((ADMIN_RENOUNCED_EVENT,), caller.clone());
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use crate::admin_transfer::propose_admin;
    use crate::{SLACalculatorContract, ADMIN_KEY};
    use soroban_sdk::testutils::{Address as _, Events as _};
    use soroban_sdk::TryIntoVal;

    #[test]
    fn renounce_clears_state_and_emits_the_renouncing_admin() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register_contract(None, SLACalculatorContract);
        let admin = Address::generate(&env);
        let pending = Address::generate(&env);

        env.as_contract(&cid, || {
            env.storage().instance().set(&ADMIN_KEY, &admin);
            propose_admin(&env, &admin, pending.clone());

            renounce_admin(&env, &admin);

            let stored_admin: Option<Address> = env.storage().instance().get(&ADMIN_KEY);
            let stored_pending: Option<Address> = env.storage().instance().get(&PENDING_ADMIN_KEY);
            let stored_ts: Option<u64> = env.storage().instance().get(&PENDING_ADMIN_TS_KEY);
            let stored_marker: Option<bool> = env.storage().instance().get(&ADMIN_RENOUNCED_KEY);
            assert_eq!(stored_admin, None);
            assert_eq!(stored_pending, None);
            assert_eq!(stored_ts, None);
            assert_eq!(stored_marker, Some(true));
        });

        let events = env.events().all();
        let (_, topics, data) = events.last().unwrap();
        let topic: Symbol = topics.get(0).unwrap().try_into_val(&env).unwrap();
        let emitter: Address = data.try_into_val(&env).unwrap();
        assert_eq!(topic, ADMIN_RENOUNCED_EVENT);
        assert_eq!(emitter, admin);
    }

    #[test]
    fn renounce_rejects_repeats_and_later_admin_mutations() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register_contract(None, SLACalculatorContract);
        let admin = Address::generate(&env);
        let successor = Address::generate(&env);

        env.as_contract(&cid, || {
            env.storage().instance().set(&ADMIN_KEY, &admin);
            renounce_admin(&env, &admin);

            let repeat = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                renounce_admin(&env, &admin);
            }));
            let propose = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                propose_admin(&env, &admin, successor);
            }));
            assert!(repeat.is_err());
            assert!(propose.is_err());

            let stored_admin: Option<Address> = env.storage().instance().get(&ADMIN_KEY);
            assert_eq!(stored_admin, None);
        });
    }
}
