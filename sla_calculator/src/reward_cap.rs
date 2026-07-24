#![no_std]

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
pub fn set_reward_cap(
    env: &Env,
    caller: &Address,
    max_rewards: i128,
) -> Result<(), SLAError> {
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

    env.events().publish(
        (EVENT_CAP_SET, EVENT_VERSION, caller),
        (max_rewards,),
    );

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

    env.events().publish(
        (EVENT_CAP_CLEARED, EVENT_VERSION, caller),
        (),
    );

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
