#![no_std]

use soroban_sdk::{symbol_short, Address, Env, Symbol};

use crate::{SLAError, ADMIN_KEY};

// -----------------------------------------------------------------------
// Storage keys
// -----------------------------------------------------------------------
const RATE_LIMIT_KEY: Symbol = symbol_short!("RLIM");

// -----------------------------------------------------------------------
// Events
// -----------------------------------------------------------------------
const EVENT_RL_SET: Symbol = symbol_short!("rl_set");
const EVENT_RL_CLR: Symbol = symbol_short!("rl_clr");
const EVENT_VERSION: Symbol = symbol_short!("v1");

// -----------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------

/// Rate limiting configuration for SLA submissions.
#[soroban_sdk::contracttype]
pub struct RateLimitConfig {
    /// Maximum submissions allowed per window.
    pub max_submissions: u32,
    /// Window duration in seconds.
    pub window_seconds: u64,
    /// Whether rate limiting is enabled.
    pub enabled: bool,
}

/// Submission tracking for rate limiting.
#[soroban_sdk::contracttype]
pub struct SubmissionTracker {
    /// Timestamp of last submission.
    pub last_submission: u64,
    /// Number of submissions in current window.
    pub count_in_window: u32,
    /// Start of current window.
    pub window_start: u64,
}

// -----------------------------------------------------------------------
// Functions
// -----------------------------------------------------------------------

/// Initialize rate limiting as disabled.
pub fn init_rate_limit(env: &Env) {
    env.storage().instance().set(
        &RATE_LIMIT_KEY,
        &RateLimitConfig {
            max_submissions: 100,
            window_seconds: 3600,
            enabled: false,
        },
    );
}

/// Configure rate limiting (admin only).
///
/// # Arguments
/// - `caller`: Must be the current admin.
/// - `max_submissions`: Maximum submissions per window.
/// - `window_seconds`: Window duration in seconds.
///
/// # Events
/// - `rl_set`: Emitted with the new configuration.
pub fn set_rate_limit(
    env: &Env,
    caller: &Address,
    max_submissions: u32,
    window_seconds: u64,
) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    if max_submissions == 0 {
        return Err(SLAError::InvalidThreshold);
    }

    if window_seconds == 0 {
        return Err(SLAError::InvalidThreshold);
    }

    env.storage().instance().set(
        &RATE_LIMIT_KEY,
        &RateLimitConfig {
            max_submissions,
            window_seconds,
            enabled: true,
        },
    );

    env.events().publish(
        (EVENT_RL_SET, EVENT_VERSION, caller),
        (max_submissions, window_seconds),
    );

    Ok(())
}

/// Disable rate limiting (admin only).
///
/// # Events
/// - `rl_clr`: Emitted when rate limiting is disabled.
pub fn disable_rate_limit(env: &Env, caller: &Address) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    let mut config: RateLimitConfig = env
        .storage()
        .instance()
        .get(&RATE_LIMIT_KEY)
        .unwrap_or(RateLimitConfig {
            max_submissions: 100,
            window_seconds: 3600,
            enabled: false,
        });

    config.enabled = false;
    env.storage().instance().set(&RATE_LIMIT_KEY, &config);

    env.events()
        .publish((EVENT_RL_CLR, EVENT_VERSION, caller), ());

    Ok(())
}

/// Check if a submission is allowed under rate limits.
///
/// Returns Ok(()) if allowed, or an error if rate limit is exceeded.
/// This should be called before processing each SLA calculation.
pub fn check_rate_limit(env: &Env, caller: &Address) -> Result<(), SLAError> {
    let config: RateLimitConfig = match env.storage().instance().get(&RATE_LIMIT_KEY) {
        Some(c) => c,
        None => return Ok(()), // No config = allow all
    };

    if !config.enabled {
        return Ok(()); // Rate limiting disabled
    }

    let now = env.ledger().timestamp();
    let tracker_key = Symbol::new(env, &format!("TRK_{:?}", caller.to_buffer()));
    let mut tracker: SubmissionTracker = env
        .storage()
        .instance()
        .get(&tracker_key)
        .unwrap_or(SubmissionTracker {
            last_submission: 0,
            count_in_window: 0,
            window_start: now,
        });

    // Check if we're in a new window
    if now.saturating_sub(tracker.window_start) >= config.window_seconds {
        // Reset window
        tracker.window_start = now;
        tracker.count_in_window = 0;
    }

    // Check rate limit
    if tracker.count_in_window >= config.max_submissions {
        return Err(SLAError::ContractPaused); // Reuse error for rate limit exceeded
    }

    Ok(())
}

/// Record a submission for rate limiting.
///
/// Called after successful SLA calculation to update the tracker.
pub fn record_submission(env: &Env, caller: &Address) {
    let config: RateLimitConfig = match env.storage().instance().get(&RATE_LIMIT_KEY) {
        Some(c) => c,
        None => return, // No config
    };

    if !config.enabled {
        return;
    }

    let now = env.ledger().timestamp();
    let tracker_key = Symbol::new(env, &format!("TRK_{:?}", caller.to_buffer()));
    let mut tracker: SubmissionTracker = env
        .storage()
        .instance()
        .get(&tracker_key)
        .unwrap_or(SubmissionTracker {
            last_submission: 0,
            count_in_window: 0,
            window_start: now,
        });

    // Check if we're in a new window
    if now.saturating_sub(tracker.window_start) >= config.window_seconds {
        tracker.window_start = now;
        tracker.count_in_window = 0;
    }

    tracker.last_submission = now;
    tracker.count_in_window = tracker.count_in_window.saturating_add(1);

    env.storage().instance().set(&tracker_key, &tracker);
}

/// Get the current rate limit configuration.
pub fn get_rate_limit_config(env: &Env) -> Result<RateLimitConfig, SLAError> {
    Ok(env
        .storage()
        .instance()
        .get(&RATE_LIMIT_KEY)
        .unwrap_or(RateLimitConfig {
            max_submissions: 100,
            window_seconds: 3600,
            enabled: false,
        }))
}

/// Get submission count for a caller in the current window.
pub fn get_submission_count(env: &Env, caller: &Address) -> Result<u32, SLAError> {
    let config: RateLimitConfig = match env.storage().instance().get(&RATE_LIMIT_KEY) {
        Some(c) => c,
        None => return Ok(0),
    };

    let now = env.ledger().timestamp();
    let tracker_key = Symbol::new(env, &format!("TRK_{:?}", caller.to_buffer()));
    let tracker: SubmissionTracker = match env.storage().instance().get(&tracker_key) {
        Some(t) => t,
        None => return Ok(0),
    };

    // Check if we're in a new window
    if now.saturating_sub(tracker.window_start) >= config.window_seconds {
        return Ok(0);
    }

    Ok(tracker.count_in_window)
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
