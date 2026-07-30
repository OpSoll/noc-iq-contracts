

use soroban_sdk::{symbol_short, Address, Env, Symbol};

use crate::{SLAError, ADMIN_KEY, STATS_KEY, SLAStats};

// -----------------------------------------------------------------------
// Storage keys
// -----------------------------------------------------------------------
const TUNING_KEY: Symbol = symbol_short!("TUNE");

// -----------------------------------------------------------------------
// Events
// -----------------------------------------------------------------------
const EVENT_TUNE_SET: Symbol = symbol_short!("tune_set");
const EVENT_TUNE_APPLY: Symbol = symbol_short!("tune_ap");
const EVENT_VERSION: Symbol = symbol_short!("v1");

// -----------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------

/// Adaptive tuning configuration.
#[soroban_sdk::contracttype]
pub struct TuningConfig {
    /// Target compliance rate (basis points, e.g., 9500 = 95%).
    pub target_compliance_bps: u32,
    /// Adjustment step size (basis points).
    pub adjustment_step_bps: u32,
    /// Maximum adjustment per period.
    pub max_adjustment_bps: u32,
    /// Evaluation window in seconds.
    pub evaluation_window: u64,
    /// Whether adaptive tuning is enabled.
    pub enabled: bool,
}

/// Tuning recommendation.
#[soroban_sdk::contracttype]
pub struct TuningRecommendation {
    /// Current compliance rate.
    pub current_compliance_bps: u32,
    /// Target compliance rate.
    pub target_compliance_bps: u32,
    /// Gap to target.
    pub gap_bps: i32,
    /// Recommended threshold adjustment.
    pub threshold_adjustment: i32,
    /// Recommended penalty adjustment.
    pub penalty_adjustment: i32,
    /// Whether adjustment is recommended.
    pub should_adjust: bool,
}

// -----------------------------------------------------------------------
// Functions
// -----------------------------------------------------------------------

/// Initialize adaptive tuning as disabled.
pub fn init_tuning(env: &Env) {
    env.storage().instance().set(
        &TUNING_KEY,
        &TuningConfig {
            target_compliance_bps: 9500,
            adjustment_step_bps: 100,
            max_adjustment_bps: 500,
            evaluation_window: 86400,
            enabled: false,
        },
    );
}

/// Configure adaptive tuning (admin only).
pub fn set_tuning(
    env: &Env,
    caller: &Address,
    target_compliance_bps: u32,
    adjustment_step_bps: u32,
    max_adjustment_bps: u32,
    evaluation_window: u64,
) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    if target_compliance_bps > 10000 {
        return Err(SLAError::InvalidThreshold);
    }

    env.storage().instance().set(
        &TUNING_KEY,
        &TuningConfig {
            target_compliance_bps,
            adjustment_step_bps,
            max_adjustment_bps,
            evaluation_window,
            enabled: true,
        },
    );

    env.events().publish(
        (EVENT_TUNE_SET, EVENT_VERSION, caller),
        (target_compliance_bps, adjustment_step_bps),
    );

    Ok(())
}

/// Disable adaptive tuning (admin only).
pub fn disable_tuning(env: &Env, caller: &Address) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    let mut config: TuningConfig = env
        .storage()
        .instance()
        .get(&TUNING_KEY)
        .unwrap_or(TuningConfig {
            target_compliance_bps: 9500,
            adjustment_step_bps: 100,
            max_adjustment_bps: 500,
            evaluation_window: 86400,
            enabled: false,
        });

    config.enabled = false;
    env.storage().instance().set(&TUNING_KEY, &config);

    Ok(())
}

/// Generate tuning recommendation based on current stats.
pub fn get_tuning_recommendation(
    env: &Env,
) -> Result<TuningRecommendation, SLAError> {
    let config: TuningConfig = env
        .storage()
        .instance()
        .get(&TUNING_KEY)
        .unwrap_or(TuningConfig {
            target_compliance_bps: 9500,
            adjustment_step_bps: 100,
            max_adjustment_bps: 500,
            evaluation_window: 86400,
            enabled: false,
        });

    let stats: SLAStats = env
        .storage()
        .instance()
        .get(&STATS_KEY)
        .unwrap_or(SLAStats {
            total_calculations: 0,
            total_violations: 0,
            total_rewards: 0,
            total_penalties: 0,
        });

    // Calculate current compliance rate
    let current_compliance_bps = if stats.total_calculations > 0 {
        let met_count = stats.total_calculations - stats.total_violations;
        ((met_count as u64 * 10000) / stats.total_calculations as u64) as u32
    } else {
        0
    };

    let gap_bps = config.target_compliance_bps as i32 - current_compliance_bps as i32;

    // Determine adjustments
    let should_adjust = config.enabled && gap_bps.unsigned_abs() > config.adjustment_step_bps;
    let mut threshold_adjustment: i32 = 0;
    let mut penalty_adjustment: i32 = 0;

    if should_adjust {
        if gap_bps > 0 {
            // Below target: loosen thresholds or increase penalties
            threshold_adjustment = config.adjustment_step_bps as i32;
            penalty_adjustment = -(config.adjustment_step_bps as i32);
        } else {
            // Above target: tighten thresholds or decrease penalties
            threshold_adjustment = -(config.adjustment_step_bps as i32);
            penalty_adjustment = config.adjustment_step_bps as i32;
        }

        // Cap adjustments
        threshold_adjustment = threshold_adjustment
            .max(-(config.max_adjustment_bps as i32))
            .min(config.max_adjustment_bps as i32);
        penalty_adjustment = penalty_adjustment
            .max(-(config.max_adjustment_bps as i32))
            .min(config.max_adjustment_bps as i32);
    }

    Ok(TuningRecommendation {
        current_compliance_bps,
        target_compliance_bps: config.target_compliance_bps,
        gap_bps,
        threshold_adjustment,
        penalty_adjustment,
        should_adjust,
    })
}

/// Get the tuning configuration.
pub fn get_tuning_config(
    env: &Env,
) -> Result<TuningConfig, SLAError> {
    Ok(env
        .storage()
        .instance()
        .get(&TUNING_KEY)
        .unwrap_or(TuningConfig {
            target_compliance_bps: 9500,
            adjustment_step_bps: 100,
            max_adjustment_bps: 500,
            evaluation_window: 86400,
            enabled: false,
        }))
}

/// Record that tuning was applied.
pub fn record_tuning_application(
    env: &Env,
    caller: &Address,
    threshold_adjustment: i32,
    penalty_adjustment: i32,
) {
    env.events().publish(
        (EVENT_TUNE_APPLY, EVENT_VERSION, caller),
        (threshold_adjustment, penalty_adjustment),
    );
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
