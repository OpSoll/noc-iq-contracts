use soroban_sdk::{symbol_short, Address, Env, Symbol};

use crate::{SLAError, ADMIN_KEY};

// -----------------------------------------------------------------------
// Storage keys
// -----------------------------------------------------------------------
const REWARD_CAP_KEY: Symbol = symbol_short!("RWDCAP");

// -----------------------------------------------------------------------
// Events
// -----------------------------------------------------------------------
const EVENT_CAP_SET: Symbol = symbol_short!("cap_set");
const EVENT_CAP_CLEARED: Symbol = symbol_short!("cap_clr");
const EVENT_VERSION: Symbol = symbol_short!("v1");

// -----------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------

/// Reward cap configuration.
#[soroban_sdk::contracttype]
pub struct RewardCap {
    /// Maximum total rewards that can be distributed. None means uncapped.
    pub max_total_rewards: Option<i128>,
    /// Current sum of all rewards distributed.
    pub total_rewards_distributed: i128,
    /// Whether the cap is currently active.
    pub active: bool,
}

// -----------------------------------------------------------------------
// Functions
// -----------------------------------------------------------------------

/// Initialize the reward cap as uncapped.
pub fn init_reward_cap(env: &Env) {
    env.storage().instance().set(
        &REWARD_CAP_KEY,
        &RewardCap {
            max_total_rewards: None,
            total_rewards_distributed: 0,
            active: false,
        },
    );
}

/// Set the maximum total rewards that can be distributed (admin only).
///
/// # Arguments
/// - `caller`: Must be the current admin.
/// - `max_rewards`: Maximum total rewards (must be positive).
///
/// # Events
/// - `cap_set`: Emitted with the new cap value.
///
/// # Errors
/// Returns `Unauthorized` if caller is not admin, or `InvalidReward`
/// if max_rewards is not positive.
pub fn set_reward_cap(env: &Env, caller: &Address, max_rewards: i128) -> Result<(), SLAError> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&ADMIN_KEY)
        .ok_or(SLAError::NotInitialized)?;
    if *caller != admin {
        return Err(SLAError::Unauthorized);
    }

    if max_rewards <= 0 {
        return Err(SLAError::InvalidReward);
    }

    let mut cap: RewardCap = env
        .storage()
        .instance()
        .get(&REWARD_CAP_KEY)
        .unwrap_or(RewardCap {
            max_total_rewards: None,
            total_rewards_distributed: 0,
            active: true,
        });

    cap.max_total_rewards = Some(max_rewards);
    cap.active = true;
    env.storage().instance().set(&REWARD_CAP_KEY, &cap);

    env.events()
        .publish((EVENT_CAP_SET, EVENT_VERSION, caller), (max_rewards,));

    Ok(())
}

/// Clear the reward cap (admin only). Rewards become unlimited.
///
/// # Arguments
/// - `caller`: Must be the current admin.
///
/// # Events
/// - `cap_clr`: Emitted when cap is cleared.
pub fn clear_reward_cap(env: &Env, caller: &Address) -> Result<(), SLAError> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&ADMIN_KEY)
        .ok_or(SLAError::NotInitialized)?;
    if *caller != admin {
        return Err(SLAError::Unauthorized);
    }

    let mut cap: RewardCap = env
        .storage()
        .instance()
        .get(&REWARD_CAP_KEY)
        .unwrap_or(RewardCap {
            max_total_rewards: None,
            total_rewards_distributed: 0,
            active: false,
        });

    cap.max_total_rewards = None;
    cap.active = false;
    env.storage().instance().set(&REWARD_CAP_KEY, &cap);

    env.events()
        .publish((EVENT_CAP_CLEARED, EVENT_VERSION, caller), ());

    Ok(())
}

/// Check whether a reward amount would exceed the cap.
///
/// Returns Ok(()) if the reward is allowed, or an error if it would exceed.
/// This should be called before distributing rewards.
pub fn check_reward_cap(env: &Env, amount: i128) -> Result<(), SLAError> {
    let cap: RewardCap = match env.storage().instance().get(&REWARD_CAP_KEY) {
        Some(c) => c,
        None => return Ok(()), // No cap configured
    };

    if !cap.active || cap.max_total_rewards.is_none() {
        return Ok(()); // Cap not active
    }

    let max = cap.max_total_rewards.unwrap();
    let new_total = cap.total_rewards_distributed.saturating_add(amount);

    if new_total > max {
        return Err(SLAError::InvalidRewardAmount);
    }

    Ok(())
}

/// Record a reward distribution (update the running total).
pub fn record_reward(env: &Env, amount: i128) {
    if amount <= 0 {
        return; // Only record positive rewards
    }

    let mut cap: RewardCap = env
        .storage()
        .instance()
        .get(&REWARD_CAP_KEY)
        .unwrap_or(RewardCap {
            max_total_rewards: None,
            total_rewards_distributed: 0,
            active: false,
        });

    cap.total_rewards_distributed = cap.total_rewards_distributed.saturating_add(amount);
    env.storage().instance().set(&REWARD_CAP_KEY, &cap);
}

/// Returns the current reward cap status.
pub fn get_reward_cap(env: &Env) -> Result<Option<RewardCap>, SLAError> {
    Ok(env.storage().instance().get(&REWARD_CAP_KEY))
}

/// Returns how much reward budget remains before hitting the cap.
/// Returns None if no cap is set.
pub fn get_remaining_reward_budget(env: &Env) -> Result<Option<i128>, SLAError> {
    let cap: RewardCap = match env.storage().instance().get(&REWARD_CAP_KEY) {
        Some(c) => c,
        None => return Ok(None),
    };

    if !cap.active || cap.max_total_rewards.is_none() {
        return Ok(None);
    }

    let remaining = cap.max_total_rewards.unwrap() - cap.total_rewards_distributed;
    Ok(Some(remaining))
}

/// View returning the configured reward cap, or `None` when rewards are
/// uncapped (never configured, or cleared).
///
/// This is the value [`check_reward_cap`] enforces: `Some(max)` means
/// distributions that would push the running total past `max` are refused,
/// `None` means nothing is refused.
pub fn configured_reward_cap(env: &Env) -> Option<i128> {
    let cap: RewardCap = env.storage().instance().get(&REWARD_CAP_KEY)?;
    if !cap.active {
        return None;
    }
    cap.max_total_rewards
}

#[cfg(test)]
mod tests {
    // The crate root is `#![no_std]`; tests opt back into std the same way
    // `benchmarks.rs` and `proptests.rs` do.
    extern crate std;

    use super::*;
    use soroban_sdk::{contract, contractimpl, testutils::Address as _};

    /// `env.storage()` and `env.events()` only run inside a contract
    /// invocation (`assert_in_contract!`), so the module's functions have to
    /// be reached through a client rather than called directly from a test.
    #[contract]
    struct RewardCapHarness;

    #[contractimpl]
    impl RewardCapHarness {
        /// Register `admin` and initialize the cap as uncapped.
        pub fn setup(env: Env, admin: Address) {
            env.storage().instance().set(&ADMIN_KEY, &admin);
            init_reward_cap(&env);
        }

        pub fn set_cap(env: Env, caller: Address, max_rewards: i128) -> Result<(), SLAError> {
            set_reward_cap(&env, &caller, max_rewards)
        }

        pub fn clear_cap(env: Env, caller: Address) -> Result<(), SLAError> {
            clear_reward_cap(&env, &caller)
        }

        pub fn check(env: Env, amount: i128) -> Result<(), SLAError> {
            check_reward_cap(&env, amount)
        }

        pub fn record(env: Env, amount: i128) {
            record_reward(&env, amount);
        }

        pub fn remaining(env: Env) -> Option<i128> {
            get_remaining_reward_budget(&env).unwrap_or(None)
        }

        pub fn configured_cap(env: Env) -> Option<i128> {
            configured_reward_cap(&env)
        }
    }

    fn harness(env: &Env) -> RewardCapHarnessClient {
        RewardCapHarnessClient::new(env, &env.register_contract(None, RewardCapHarness))
    }

    /// Fresh environment with `admin` registered and `max_rewards` configured.
    fn setup_with_cap(env: &Env, max_rewards: i128) -> (Address, RewardCapHarnessClient) {
        let admin = Address::generate(env);
        let client = harness(env);
        client.setup(&admin);
        if max_rewards > 0 {
            client.set_cap(&admin, &max_rewards);
        }
        (admin, client)
    }

    // -------------------------------------------------------------------
    // #914 - reward cap enforcement
    // -------------------------------------------------------------------

    /// #914: distributions are capped at the configured maximum - the
    /// running total stops exactly at the cap and never crosses it.
    #[test]
    fn rewards_never_exceed_the_configured_cap() {
        let env = Env::default();
        let (_, client) = setup_with_cap(&env, 1_000);

        assert!(client.try_check(&600).is_ok());
        client.record(&600);
        assert!(client.try_check(&400).is_ok(), "exactly at the cap is fine");
        client.record(&400); // total is now exactly the cap

        // One more unit would cross the cap and is refused.
        assert!(matches!(
            client.try_check(&1),
            Err(Ok(SLAError::InvalidRewardAmount))
        ));
        assert_eq!(
            client.remaining(),
            Some(0),
            "budget is exhausted, not negative"
        );
    }

    #[test]
    fn amounts_that_would_cross_the_cap_are_rejected() {
        let env = Env::default();
        let (_, client) = setup_with_cap(&env, 100);

        assert!(client.try_check(&100).is_ok(), "exactly at cap is allowed");
        assert!(matches!(
            client.try_check(&101),
            Err(Ok(SLAError::InvalidRewardAmount))
        ));

        client.record(&100);
        assert!(
            matches!(client.try_check(&1), Err(Ok(SLAError::InvalidRewardAmount))),
            "exhausted budget refuses even one unit"
        );
    }

    #[test]
    fn rewards_are_unlimited_without_an_active_cap() {
        let env = Env::default();
        let (admin, client) = setup_with_cap(&env, 0); // initialized, never capped

        assert!(client.try_check(&i128::MAX).is_ok(), "no cap configured");

        // A configured cap is enforced...
        client.set_cap(&admin, &500);
        assert!(matches!(
            client.try_check(&501),
            Err(Ok(SLAError::InvalidRewardAmount))
        ));

        // ...and clearing it restores unlimited rewards.
        client.clear_cap(&admin);
        assert!(client.try_check(&501).is_ok());
        assert_eq!(client.configured_cap(), None);
    }

    #[test]
    fn remaining_budget_shrinks_with_every_recorded_reward() {
        let env = Env::default();
        let (_, client) = setup_with_cap(&env, 300);

        assert_eq!(client.remaining(), Some(300));
        client.record(&100);
        assert_eq!(client.remaining(), Some(200));
        client.record(&200);
        assert_eq!(client.remaining(), Some(0));
        assert!(matches!(
            client.try_check(&1),
            Err(Ok(SLAError::InvalidRewardAmount))
        ));
    }

    #[test]
    fn only_the_admin_can_configure_the_cap() {
        let env = Env::default();
        let (admin, client) = setup_with_cap(&env, 0);
        let attacker = Address::generate(&env);

        assert!(matches!(
            client.try_set_cap(&attacker, &500),
            Err(Ok(SLAError::Unauthorized))
        ));
        assert_eq!(client.configured_cap(), None, "cap unchanged by attacker");

        client.set_cap(&admin, &500);
        assert!(matches!(
            client.try_clear_cap(&attacker),
            Err(Ok(SLAError::Unauthorized))
        ));
        assert_eq!(
            client.configured_cap(),
            Some(500),
            "cap unchanged by attacker"
        );
    }

    #[test]
    fn the_cap_cannot_be_set_to_a_non_positive_value() {
        let env = Env::default();
        let (admin, client) = setup_with_cap(&env, 0);

        assert!(matches!(
            client.try_set_cap(&admin, &0),
            Err(Ok(SLAError::InvalidReward))
        ));
        assert!(matches!(
            client.try_set_cap(&admin, &-1),
            Err(Ok(SLAError::InvalidReward))
        ));
        assert_eq!(client.configured_cap(), None);
    }

    // -------------------------------------------------------------------
    // #915 - configured cap view
    // -------------------------------------------------------------------

    /// #915: the view returns the configured cap.
    #[test]
    fn view_reports_the_configured_cap() {
        let env = Env::default();
        let (_, client) = setup_with_cap(&env, 2_500);
        assert_eq!(client.configured_cap(), Some(2_500));
    }

    #[test]
    fn view_is_none_before_configuration_and_after_clearing() {
        let env = Env::default();
        let (admin, client) = setup_with_cap(&env, 0);
        assert_eq!(client.configured_cap(), None, "never configured");

        client.set_cap(&admin, &700);
        assert_eq!(client.configured_cap(), Some(700));

        client.clear_cap(&admin);
        assert_eq!(client.configured_cap(), None, "cleared");
    }

    /// The view describes exactly what enforcement does: `Some(max)` iff a
    /// distribution past `max` would be refused; `None` iff nothing is
    /// refused.
    #[test]
    fn the_view_describes_exactly_the_enforced_range() {
        let env = Env::default();
        let (admin, client) = setup_with_cap(&env, 0);

        for max in [1i128, 99, 10_000] {
            client.set_cap(&admin, &max);
            assert_eq!(client.configured_cap(), Some(max));
            assert!(client.try_check(&max).is_ok(), "at the cap is allowed");
            assert!(
                matches!(
                    client.try_check(&(max + 1)),
                    Err(Ok(SLAError::InvalidRewardAmount))
                ),
                "one past the cap is refused"
            );
        }

        client.clear_cap(&admin);
        assert!(
            client.try_check(&i128::MAX).is_ok(),
            "uncapped: nothing refused"
        );
    }
}
