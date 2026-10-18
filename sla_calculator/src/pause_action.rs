use soroban_sdk::{contracttype, symbol_short, Address, Env, String, Symbol};

const PAUSED_KEY: Symbol = symbol_short!("PAUSED");
const PAUSE_INFO_KEY: Symbol = symbol_short!("PAUSEINF");
const CONTRACT_PAUSED_EVENT: Symbol = symbol_short!("paused");

#[contracttype]
#[derive(Clone)]
pub struct PauseInfo {
    pub reason: String,
    pub paused_at: u64,
    pub paused_by: Address,
}

/// Admin-only: pauses the contract with an explanatory reason.
pub fn pause(env: &Env, admin: &Address, reason: String) {
    admin.require_auth();
    let info = PauseInfo {
        reason,
        paused_at: env.ledger().timestamp(),
        paused_by: admin.clone(),
    };
    env.storage().instance().set(&PAUSED_KEY, &true);
    env.storage().instance().set(&PAUSE_INFO_KEY, &info);
    env.events()
        .publish((CONTRACT_PAUSED_EVENT,), admin.clone());
}

/// View returning the metadata of the most recent pause action.
///
/// Returns `None` while the contract has never been paused. The result
/// carries the actor that authorized the pause (`paused_by`) and the ledger
/// timestamp it was recorded at (`paused_at`).
///
/// The keys written by [`pause`] are the same ones the contract's own
/// `get_pause_info` entrypoint reads, so the view stays consistent with
/// whichever path recorded the pause.
pub fn last_pause_action(env: &Env) -> Option<PauseInfo> {
    env.storage().instance().get(&PAUSE_INFO_KEY)
}

#[cfg(test)]
mod tests {
    // The crate root is `#![no_std]`; tests opt back into std the same way
    // `benchmarks.rs` and `proptests.rs` do.
    extern crate std;

    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger as _};
    use soroban_sdk::{contract, contractimpl};

    /// `require_auth` and `env.storage()` only run inside a contract
    /// invocation, so the module's functions have to be reached through a
    /// client rather than called directly from a test.
    #[contract]
    struct PauseActionHarness;

    #[contractimpl]
    impl PauseActionHarness {
        pub fn do_pause(env: Env, admin: Address, reason: String) {
            pause(&env, &admin, reason);
        }

        pub fn last_action(env: Env) -> Option<PauseInfo> {
            last_pause_action(&env)
        }

        pub fn paused_flag(env: Env) -> bool {
            env.storage().instance().get(&PAUSED_KEY).unwrap_or(false)
        }
    }

    fn harness(env: &Env) -> PauseActionHarnessClient {
        PauseActionHarnessClient::new(env, &env.register_contract(None, PauseActionHarness))
    }

    fn reason(env: &Env) -> String {
        String::from_str(env, "planned maintenance")
    }

    /// #890 – a caller that cannot prove admin authorization must be rejected.
    #[test]
    #[should_panic]
    fn pause_without_admin_authorization_is_rejected() {
        let env = Env::default();
        // No auth proofs are available, so `require_auth` cannot succeed.
        env.mock_auths(&[]);
        let client = harness(&env);
        let admin = Address::generate(&env);
        client.do_pause(&admin, &reason(&env));
    }

    #[test]
    fn rejected_pause_records_nothing() {
        let env = Env::default();
        env.mock_auths(&[]);
        let client = harness(&env);
        let admin = Address::generate(&env);

        // The unauthorized attempt panics inside `require_auth`; catch it so we
        // can assert the state it guards is untouched.
        let attempt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.do_pause(&admin, &reason(&env));
        }));
        assert!(attempt.is_err(), "unauthorized pause should have panicked");
        assert!(client.last_action().is_none());
        assert!(!client.paused_flag());
    }

    #[test]
    fn authorized_pause_records_actor_and_ledger() {
        let env = Env::default();
        env.mock_all_auths();
        let client = harness(&env);
        let admin = Address::generate(&env);
        env.ledger().with_mut(|l| l.timestamp = 1_700_000_123);

        client.do_pause(&admin, &reason(&env));

        let last = client.last_action().expect("a pause action was recorded");
        assert_eq!(last.paused_by, admin);
        assert_eq!(last.paused_at, 1_700_000_123);
        assert_eq!(last.reason, reason(&env));
        assert!(client.paused_flag());
    }

    #[test]
    fn last_pause_action_is_none_before_any_pause() {
        let env = Env::default();
        let client = harness(&env);
        assert!(client.last_action().is_none());
    }

    #[test]
    fn the_latest_pause_action_supersedes_earlier_ones() {
        let env = Env::default();
        env.mock_all_auths();
        let client = harness(&env);
        let first = Address::generate(&env);
        let second = Address::generate(&env);

        env.ledger().with_mut(|l| l.timestamp = 1_000);
        client.do_pause(&first, &String::from_str(&env, "first"));
        env.ledger().with_mut(|l| l.timestamp = 2_000);
        client.do_pause(&second, &String::from_str(&env, "second"));

        let last = client.last_action().expect("pause action recorded");
        assert_eq!(last.paused_by, second);
        assert_eq!(last.paused_at, 2_000);
        assert_eq!(last.reason, String::from_str(&env, "second"));
    }
}
