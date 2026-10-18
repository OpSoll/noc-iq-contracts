use soroban_sdk::{contracttype, symbol_short, Address, Env, String, Symbol};

const PAUSED_KEY: Symbol = symbol_short!("PAUSED");
const PAUSE_INFO_KEY: Symbol = symbol_short!("PAUSEINF");

#[contracttype]
#[derive(Clone)]
pub struct PauseInfo {
    pub reason: String,
    pub paused_at: u64,
    pub paused_by: Address,
}

/// Returns whether the contract is currently paused.
pub fn is_paused(env: &Env) -> bool {
    env.storage().instance().get(&PAUSED_KEY).unwrap_or(false)
}

/// Returns the stored pause metadata, if the contract is paused.
pub fn get_pause_info(env: &Env) -> Option<PauseInfo> {
    env.storage().instance().get(&PAUSE_INFO_KEY)
}

/// Structured pause status: the paused flag together with the reason and the
/// ledger timestamp of the pause it refers to.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PauseStatus {
    /// Whether the contract is currently paused.
    pub paused: bool,
    /// Why the contract is paused; empty while unpaused.
    pub reason: String,
    /// Ledger timestamp when the current pause was recorded; 0 while unpaused.
    pub timestamp: u64,
}

/// View returning the structured pause status (`paused`, `reason`,
/// `timestamp`) instead of the bare boolean [`is_paused`] exposes.
///
/// While unpaused the reason and timestamp are empty/0 - the view describes
/// the *current* pause, not the last historical one. While paused but without
/// stored metadata the flag still reports `paused` with empty details, so a
/// consumer never silently flips to "not paused".
pub fn pause_status(env: &Env) -> PauseStatus {
    if !is_paused(env) {
        return PauseStatus {
            paused: false,
            reason: String::from_str(env, ""),
            timestamp: 0,
        };
    }
    match get_pause_info(env) {
        Some(info) => PauseStatus {
            paused: true,
            reason: info.reason,
            timestamp: info.paused_at,
        },
        None => PauseStatus {
            paused: true,
            reason: String::from_str(env, ""),
            timestamp: 0,
        },
    }
}

#[cfg(test)]
mod tests {
    // The crate root is `#![no_std]`; tests opt back into std the same way
    // `benchmarks.rs` and `proptests.rs` do.
    extern crate std;

    use super::*;
    use soroban_sdk::{contract, contractimpl, testutils::Address as _};

    /// `env.storage()` only runs inside a contract invocation
    /// (`assert_in_contract!`), so the module's view has to be reached through
    /// a client rather than called directly from a test. This harness also
    /// seeds the state the view reads.
    #[contract]
    struct PauseStatusHarness;

    #[contractimpl]
    impl PauseStatusHarness {
        /// Record a pause exactly the way a pause action would: flag set,
        /// metadata stored.
        pub fn seed_paused(env: Env, actor: Address, reason: String, at: u64) {
            env.storage().instance().set(&PAUSED_KEY, &true);
            env.storage().instance().set(
                &PAUSE_INFO_KEY,
                &PauseInfo {
                    reason,
                    paused_at: at,
                    paused_by: actor,
                },
            );
        }

        /// Degenerate state: pause flag set but metadata missing.
        pub fn seed_flag_only(env: Env) {
            env.storage().instance().set(&PAUSED_KEY, &true);
        }

        /// Clear both the flag and the metadata, as unpause does.
        pub fn clear(env: Env) {
            env.storage().instance().set(&PAUSED_KEY, &false);
            env.storage().instance().remove(&PAUSE_INFO_KEY);
        }

        pub fn status(env: Env) -> PauseStatus {
            pause_status(&env)
        }

        pub fn flag(env: Env) -> bool {
            is_paused(&env)
        }

        pub fn info(env: Env) -> Option<PauseInfo> {
            get_pause_info(&env)
        }
    }

    fn harness(env: &Env) -> PauseStatusHarnessClient {
        PauseStatusHarnessClient::new(env, &env.register_contract(None, PauseStatusHarness))
    }

    /// #893: the view returns the paused flag, the reason and the timestamp.
    #[test]
    fn status_returns_flag_reason_and_timestamp() {
        let env = Env::default();
        let client = harness(&env);
        let actor = Address::generate(&env);

        client.seed_paused(
            &actor,
            &String::from_str(&env, "upstream provider incident"),
            &1_700_000_456,
        );

        let status = client.status();
        assert!(status.paused);
        assert_eq!(
            status.reason,
            String::from_str(&env, "upstream provider incident")
        );
        assert_eq!(status.timestamp, 1_700_000_456);
    }

    #[test]
    fn status_reports_no_pause_before_any_pause() {
        let env = Env::default();
        let client = harness(&env);

        let status = client.status();
        assert!(!status.paused);
        assert_eq!(status.reason, String::from_str(&env, ""));
        assert_eq!(status.timestamp, 0);
    }

    #[test]
    fn clearing_the_pause_clears_the_status() {
        let env = Env::default();
        let client = harness(&env);
        let actor = Address::generate(&env);

        client.seed_paused(&actor, &String::from_str(&env, "maintenance"), &5_000);
        assert!(client.status().paused);

        client.clear();

        let status = client.status();
        assert!(!status.paused);
        assert_eq!(status.reason, String::from_str(&env, ""));
        assert_eq!(status.timestamp, 0);
    }

    #[test]
    fn pause_flag_without_metadata_still_reports_paused() {
        // Defensive: the flag is authoritative, so a consumer never sees
        // "not paused" while the flag says otherwise.
        let env = Env::default();
        let client = harness(&env);

        client.seed_flag_only();

        assert!(client.flag());
        let status = client.status();
        assert!(status.paused);
        assert_eq!(status.reason, String::from_str(&env, ""));
        assert_eq!(status.timestamp, 0);
    }

    #[test]
    fn structured_status_agrees_with_the_existing_views() {
        // The new view is a superset of the existing ones - it must never
        // contradict them.
        let env = Env::default();
        let client = harness(&env);
        let actor = Address::generate(&env);

        // Unpaused: agrees with `is_paused`.
        assert_eq!(client.status().paused, client.flag());

        // Paused: flag and metadata both line up.
        client.seed_paused(&actor, &String::from_str(&env, "incident"), &9_000);
        let status = client.status();
        assert_eq!(status.paused, client.flag());
        let info = client.info().expect("metadata seeded");
        assert_eq!(status.reason, info.reason);
        assert_eq!(status.timestamp, info.paused_at);
    }
}
