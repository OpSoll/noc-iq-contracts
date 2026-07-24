#![no_std]

use soroban_sdk::{symbol_short, Address, Env, Symbol};

use crate::{SLAError, ADMIN_KEY};

// -----------------------------------------------------------------------
// Storage keys
// -----------------------------------------------------------------------
const SUB_RATE_KEY: Symbol = symbol_short!("SRATE");

// -----------------------------------------------------------------------
// Events
// -----------------------------------------------------------------------
const EVENT_SR_SET: Symbol = symbol_short!("sr_set");
const EVENT_SR_CLR: Symbol = symbol_short!("sr_clr");
const EVENT_VERSION: Symbol = symbol_short!("v1");

// -----------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------

/// Submission rate limiting configuration.
#[soroban_sdk::contracttype]
pub struct SubmissionRateConfig {
    /// Maximum submissions per operator per window.
    pub max_per_operator: u32,
    /// Maximum total submissions per window.
    pub max_total: u32,
    /// Window duration in seconds.
    pub window_seconds: u64,
    /// Whether rate limiting is enabled.
    pub enabled: bool,
}

/// Per-operator submission tracking.
#[soroban_sdk::contracttype]
pub struct OperatorSubmissionTrack {
    /// Operator address.
    pub operator: Address,
    /// Submissions in current window.
    pub count: u32,
    /// Window start timestamp.
    pub window_start: u64,
}

/// Global submission tracking.
#[soroban_sdk::contracttype]
pub struct GlobalSubmissionTrack {
    /// Total submissions in current window.
    pub total_count: u32,
    /// Window start timestamp.
    pub window_start: u64,
}

// -----------------------------------------------------------------------
// Functions
// -----------------------------------------------------------------------

/// Initialize submission rate limiting as disabled.
pub fn init_submission_rate(env: &Env) {
    env.storage().instance().set(
        &SUB_RATE_KEY,
        &SubmissionRateConfig {
            max_per_operator: 50,
            max_total: 200,
            window_seconds: 3600,
            enabled: false,
        },
    );
}

/// Configure submission rate limiting (admin only).
///
/// # Arguments
/// - `caller`: Must be the current admin.
/// - `max_per_operator`: Maximum submissions per operator per window.
/// - `max_total`: Maximum total submissions per window.
/// - `window_seconds`: Window duration in seconds.
///
/// # Events
/// - `sr_set`: Emitted with the new configuration.
pub fn set_submission_rate(
    env: &Env,
    caller: &Address,
    max_per_operator: u32,
    max_total: u32,
    window_seconds: u64,
) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    if max_per_operator == 0 || max_total == 0 || window_seconds == 0 {
        return Err(SLAError::InvalidThreshold);
    }

    if max_per_operator > max_total {
        return Err(SLAError::InvalidThreshold);
    }

    env.storage().instance().set(
        &SUB_RATE_KEY,
        &SubmissionRateConfig {
            max_per_operator,
            max_total,
            window_seconds,
            enabled: true,
        },
    );

    env.events().publish(
        (EVENT_SR_SET, EVENT_VERSION, caller),
        (max_per_operator, max_total, window_seconds),
    );

    Ok(())
}

/// Disable submission rate limiting (admin only).
///
/// # Events
/// - `sr_clr`: Emitted when rate limiting is disabled.
pub fn disable_submission_rate(env: &Env, caller: &Address) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    let mut config: SubmissionRateConfig = env
        .storage()
        .instance()
        .get(&SUB_RATE_KEY)
        .unwrap_or(SubmissionRateConfig {
            max_per_operator: 50,
            max_total: 200,
            window_seconds: 3600,
            enabled: false,
        });

    config.enabled = false;
    env.storage().instance().set(&SUB_RATE_KEY, &config);

    env.events()
        .publish((EVENT_SR_CLR, EVENT_VERSION, caller), ());

    Ok(())
}

/// Check if a submission is allowed under rate limits.
///
/// Checks both per-operator and global limits.
pub fn check_submission_rate(
    env: &Env,
    operator: &Address,
) -> Result<(), SLAError> {
    let config: SubmissionRateConfig = match env.storage().instance().get(&SUB_RATE_KEY) {
        Some(c) => c,
        None => return Ok(()),
    };

    if !config.enabled {
        return Ok(());
    }

    let now = env.ledger().timestamp();

    // Check per-operator limit
    let op_key = Symbol::new(env, &format!("SROP_{:?}", operator.to_buffer()));
    let op_track: OperatorSubmissionTrack = env
        .storage()
        .instance()
        .get(&op_key)
        .unwrap_or(OperatorSubmissionTrack {
            operator: operator.clone(),
            count: 0,
            window_start: now,
        });

    let op_count = if now.saturating_sub(op_track.window_start) >= config.window_seconds {
        0 // New window
    } else {
        op_track.count
    };

    if op_count >= config.max_per_operator {
        return Err(SLAError::ContractPaused);
    }

    // Check global limit
    let global_track: GlobalSubmissionTrack = env
        .storage()
        .instance()
        .get(&symbol_short!("SGLOB"))
        .unwrap_or(GlobalSubmissionTrack {
            total_count: 0,
            window_start: now,
        });

    let global_count = if now.saturating_sub(global_track.window_start) >= config.window_seconds {
        0 // New window
    } else {
        global_track.total_count
    };

    if global_count >= config.max_total {
        return Err(SLAError::ContractPaused);
    }

    Ok(())
}

/// Record a submission for rate limiting.
pub fn record_submission(env: &Env, operator: &Address) {
    let config: SubmissionRateConfig = match env.storage().instance().get(&SUB_RATE_KEY) {
        Some(c) => c,
        None => return,
    };

    if !config.enabled {
        return;
    }

    let now = env.ledger().timestamp();

    // Update per-operator tracking
    let op_key = Symbol::new(env, &format!("SROP_{:?}", operator.to_buffer()));
    let mut op_track: OperatorSubmissionTrack = env
        .storage()
        .instance()
        .get(&op_key)
        .unwrap_or(OperatorSubmissionTrack {
            operator: operator.clone(),
            count: 0,
            window_start: now,
        });

    if now.saturating_sub(op_track.window_start) >= config.window_seconds {
        op_track.window_start = now;
        op_track.count = 0;
    }

    op_track.count = op_track.count.saturating_add(1);
    env.storage().instance().set(&op_key, &op_track);

    // Update global tracking
    let mut global_track: GlobalSubmissionTrack = env
        .storage()
        .instance()
        .get(&symbol_short!("SGLOB"))
        .unwrap_or(GlobalSubmissionTrack {
            total_count: 0,
            window_start: now,
        });

    if now.saturating_sub(global_track.window_start) >= config.window_seconds {
        global_track.window_start = now;
        global_track.total_count = 0;
    }

    global_track.total_count = global_track.total_count.saturating_add(1);
    env.storage().instance().set(&symbol_short!("SGLOB"), &global_track);
}

/// Get submission rate configuration.
pub fn get_submission_rate_config(
    env: &Env,
) -> Result<SubmissionRateConfig, SLAError> {
    Ok(env
        .storage()
        .instance()
        .get(&SUB_RATE_KEY)
        .unwrap_or(SubmissionRateConfig {
            max_per_operator: 50,
            max_total: 200,
            window_seconds: 3600,
            enabled: false,
        }))
}

/// Get operator's current submission count in window.
pub fn get_operator_submission_count(
    env: &Env,
    operator: &Address,
) -> Result<u32, SLAError> {
    let config: SubmissionRateConfig = match env.storage().instance().get(&SUB_RATE_KEY) {
        Some(c) => c,
        None => return Ok(0),
    };

    let now = env.ledger().timestamp();
    let op_key = Symbol::new(env, &format!("SROP_{:?}", operator.to_buffer()));
    let op_track: OperatorSubmissionTrack = match env.storage().instance().get(&op_key) {
        Some(t) => t,
        None => return Ok(0),
    };

    if now.saturating_sub(op_track.window_start) >= config.window_seconds {
        return Ok(0);
    }

    Ok(op_track.count)
}

/// Get global submission count in current window.
pub fn get_global_submission_count(env: &Env) -> Result<u32, SLAError> {
    let global_track: GlobalSubmissionTrack = match env
        .storage()
        .instance()
        .get(&symbol_short!("SGLOB"))
    {
        Some(t) => t,
        None => return Ok(0),
    };

    let now = env.ledger().timestamp();
    let config: SubmissionRateConfig = match env.storage().instance().get(&SUB_RATE_KEY) {
        Some(c) => c,
        None => return Ok(0),
    };

    if now.saturating_sub(global_track.window_start) >= config.window_seconds {
        return Ok(0);
    }

    Ok(global_track.total_count)
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
