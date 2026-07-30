#![no_std]

use soroban_sdk::{symbol_short, Address, Env, Symbol};

use crate::{SLAError, ADMIN_KEY, PAUSED_KEY, CONFIG_KEY, SLAConfig};

// -----------------------------------------------------------------------
// Storage keys
// -----------------------------------------------------------------------
const ROLLBACK_KEY: Symbol = symbol_short!("RBACK");
const PRE_ROLLBACK_CONFIG_KEY: Symbol = symbol_short!("PRCFG");

// -----------------------------------------------------------------------
// Events
// -----------------------------------------------------------------------
const EVENT_ROLLBACK_INIT: Symbol = symbol_short!("rb_init");
const EVENT_ROLLBACK_EXEC: Symbol = symbol_short!("rb_exec");
const EVENT_ROLLBACK_UNDO: Symbol = symbol_short!("rb_undo");
const EVENT_VERSION: Symbol = symbol_short!("v1");

// -----------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------

/// Emergency rollback state.
#[soroban_sdk::contracttype]
pub struct RollbackState {
    /// Whether a rollback is in progress.
    pub in_progress: bool,
    /// Timestamp when rollback was initiated.
    pub initiated_at: u64,
    /// Admin who initiated the rollback.
    pub initiated_by: Address,
    /// Reason for the rollback.
    pub reason: soroban_sdk::String,
}

// -----------------------------------------------------------------------
// Functions
// -----------------------------------------------------------------------

/// Initiate an emergency rollback.
///
/// Immediately pauses the contract and snapshots current config for restoration.
/// Only admin can initiate rollbacks.
///
/// # Arguments
/// - `caller`: Must be the current admin.
/// - `reason`: Reason for the emergency rollback.
///
/// # Events
/// - `rb_init`: Emitted when rollback is initiated.
///
/// # Effects
/// - Contract is paused
/// - Current config is snapshot for potential restoration
pub fn initiate_rollback(
    env: &Env,
    caller: &Address,
    reason: soroban_sdk::String,
) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    let now = env.ledger().timestamp();

    // Pause the contract immediately
    env.storage().instance().set(&PAUSED_KEY, &true);

    // Snapshot current config before rollback
    if let Some(configs) = env.storage().instance().get::<Symbol, soroban_sdk::Map<Symbol, SLAConfig>>(&CONFIG_KEY) {
        env.storage().instance().set(&PRE_ROLLBACK_CONFIG_KEY, &configs);
    }

    // Record rollback state
    env.storage().instance().set(
        &ROLLBACK_KEY,
        &RollbackState {
            in_progress: true,
            initiated_at: now,
            initiated_by: caller.clone(),
            reason,
        },
    );

    env.events().publish(
        (EVENT_ROLLBACK_INIT, EVENT_VERSION, caller),
        (now,),
    );

    Ok(())
}

/// Execute a rollback to default configuration.
///
/// Resets all configs to factory defaults. Requires rollback to be in progress.
///
/// # Arguments
/// - `caller`: Must be the current admin.
///
/// # Events
/// - `rb_exec`: Emitted when rollback is executed.
///
/// # Effects
/// - All severity configs reset to defaults
/// - Rollback state cleared
/// - Contract remains paused
pub fn execute_rollback(
    env: &Env,
    caller: &Address,
) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    let state: RollbackState = env
        .storage()
        .instance()
        .get(&ROLLBACK_KEY)
        .ok_or(SLAError::NotInitialized)?;

    if !state.in_progress {
        return Err(SLAError::ContractPaused);
    }

    // Reset to default configs
    let mut configs = soroban_sdk::Map::<Symbol, SLAConfig>::new(env);
    configs.set(
        symbol_short!("critical"),
        SLAConfig {
            threshold_minutes: 15,
            penalty_per_minute: 100,
            reward_base: 750, top_tier_multiplier: 200, excel_tier_multiplier: 150, good_tier_multiplier: 100,
        },
    );
    configs.set(
        symbol_short!("high"),
        SLAConfig {
            threshold_minutes: 30,
            penalty_per_minute: 50,
            reward_base: 750, top_tier_multiplier: 200, excel_tier_multiplier: 150, good_tier_multiplier: 100,
        },
    );
    configs.set(
        symbol_short!("medium"),
        SLAConfig {
            threshold_minutes: 60,
            penalty_per_minute: 25,
            reward_base: 750, top_tier_multiplier: 200, excel_tier_multiplier: 150, good_tier_multiplier: 100,
        },
    );
    configs.set(
        symbol_short!("low"),
        SLAConfig {
            threshold_minutes: 120,
            penalty_per_minute: 10,
            reward_base: 600, top_tier_multiplier: 200, excel_tier_multiplier: 150, good_tier_multiplier: 100,
        },
    );

    env.storage().instance().set(&CONFIG_KEY, &configs);

    // Clear rollback state
    env.storage().instance().remove(&ROLLBACK_KEY);
    env.storage().instance().remove(&PRE_ROLLBACK_CONFIG_KEY);

    env.events()
        .publish((EVENT_ROLLBACK_EXEC, EVENT_VERSION, caller), ());

    Ok(())
}

/// Undo a rollback, restoring pre-rollback configuration.
///
/// Only works if a rollback is in progress and config was snapshot.
///
/// # Arguments
/// - `caller`: Must be the current admin.
///
/// # Events
/// - `rb_undo`: Emitted when rollback is undone.
///
/// # Effects
/// - Config restored from pre-rollback snapshot
/// - Contract unpaused
/// - Rollback state cleared
pub fn undo_rollback(
    env: &Env,
    caller: &Address,
) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    let state: RollbackState = env
        .storage()
        .instance()
        .get(&ROLLBACK_KEY)
        .ok_or(SLAError::NotInitialized)?;

    if !state.in_progress {
        return Err(SLAError::ContractPaused);
    }

    // Restore pre-rollback config if available
    if let Some(configs) = env.storage().instance().get::<Symbol, soroban_sdk::Map<Symbol, SLAConfig>>(&PRE_ROLLBACK_CONFIG_KEY) {
        env.storage().instance().set(&CONFIG_KEY, &configs);
    }

    // Unpause the contract
    env.storage().instance().set(&PAUSED_KEY, &false);

    // Clear rollback state
    env.storage().instance().remove(&ROLLBACK_KEY);
    env.storage().instance().remove(&PRE_ROLLBACK_CONFIG_KEY);

    env.events()
        .publish((EVENT_ROLLBACK_UNDO, EVENT_VERSION, caller), ());

    Ok(())
}

/// Get the current rollback state.
pub fn get_rollback_state(
    env: &Env,
) -> Result<Option<RollbackState>, SLAError> {
    Ok(env.storage().instance().get(&ROLLBACK_KEY))
}

/// Check if a rollback is in progress.
pub fn is_rollback_in_progress(env: &Env) -> Result<bool, SLAError> {
    let state: RollbackState = match env.storage().instance().get(&ROLLBACK_KEY) {
        Some(s) => s,
        None => return Ok(false),
    };
    Ok(state.in_progress)
}

/// Helper to verify admin role.
fn require_admin(env: &Env, caller: &Address) -> Result<(), SLAError> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&ADMIN_KEY)
        .ok_or(SLAError::NotInitialized)?;
    if *caller != admin {
        return Err(SLAError::Unauthorized);
    }
    Ok(())
}
