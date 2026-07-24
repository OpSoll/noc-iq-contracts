#![no_std]

use soroban_sdk::{symbol_short, Address, Env, Symbol};

use crate::{SLAError, ADMIN_KEY};

// -----------------------------------------------------------------------
// Storage keys
// -----------------------------------------------------------------------
const PENALTY_ESC_KEY: Symbol = symbol_short!("PESC");

// -----------------------------------------------------------------------
// Events
// -----------------------------------------------------------------------
const EVENT_ESC_SET: Symbol = symbol_short!("esc_set");
const EVENT_ESC_CLR: Symbol = symbol_short!("esc_clr");
const EVENT_VERSION: Symbol = symbol_short!("v1");

// -----------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------

/// Penalty escalation configuration.
#[soroban_sdk::contracttype]
pub struct PenaltyEscalationConfig {
    /// Number of consecutive violations before escalation triggers.
    pub trigger_count: u32,
    /// Multiplier for penalty after escalation (basis points, 150 = 1.5x).
    pub escalation_multiplier_bps: u32,
    /// Whether penalty escalation is enabled.
    pub enabled: bool,
}

/// Violation tracking for a specific outage or operator.
#[soroban_sdk::contracttype]
pub struct ViolationTracker {
    /// Number of consecutive violations.
    pub consecutive_violations: u32,
    /// Timestamp of last violation.
    pub last_violation_at: u64,
    /// Whether escalation is currently active.
    pub escalated: bool,
}

// -----------------------------------------------------------------------
// Functions
// -----------------------------------------------------------------------

/// Initialize penalty escalation as disabled.
pub fn init_penalty_escalation(env: &Env) {
    env.storage().instance().set(
        &PENALTY_ESC_KEY,
        &PenaltyEscalationConfig {
            trigger_count: 3,
            escalation_multiplier_bps: 150,
            enabled: false,
        },
    );
}

/// Configure penalty escalation (admin only).
///
/// # Arguments
/// - `caller`: Must be the current admin.
/// - `trigger_count`: Number of consecutive violations before escalation.
/// - `escalation_multiplier_bps`: Penalty multiplier in basis points (150 = 1.5x).
///
/// # Events
/// - `esc_set`: Emitted with the new configuration.
pub fn set_penalty_escalation(
    env: &Env,
    caller: &Address,
    trigger_count: u32,
    escalation_multiplier_bps: u32,
) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    if trigger_count == 0 {
        return Err(SLAError::InvalidThreshold);
    }

    if escalation_multiplier_bps < 100 || escalation_multiplier_bps > 500 {
        return Err(SLAError::InvalidPenalty);
    }

    env.storage().instance().set(
        &PENALTY_ESC_KEY,
        &PenaltyEscalationConfig {
            trigger_count,
            escalation_multiplier_bps,
            enabled: true,
        },
    );

    env.events().publish(
        (EVENT_ESC_SET, EVENT_VERSION, caller),
        (trigger_count, escalation_multiplier_bps),
    );

    Ok(())
}

/// Disable penalty escalation (admin only).
///
/// # Events
/// - `esc_clr`: Emitted when penalty escalation is disabled.
pub fn disable_penalty_escalation(env: &Env, caller: &Address) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    let mut config: PenaltyEscalationConfig = env
        .storage()
        .instance()
        .get(&PENALTY_ESC_KEY)
        .unwrap_or(PenaltyEscalationConfig {
            trigger_count: 3,
            escalation_multiplier_bps: 150,
            enabled: false,
        });

    config.enabled = false;
    env.storage().instance().set(&PENALTY_ESC_KEY, &config);

    env.events()
        .publish((EVENT_ESC_CLR, EVENT_VERSION, caller), ());

    Ok(())
}

/// Apply penalty escalation to a penalty amount.
///
/// Checks violation history and applies multiplier if escalation is triggered.
///
/// # Arguments
/// - `outage_id`: The outage ID to check for escalation.
/// - `base_penalty`: The base penalty amount (positive).
///
/// # Returns
/// The adjusted penalty amount (positive, may be escalated).
pub fn apply_penalty_escalation(
    env: &Env,
    outage_id: &Symbol,
    base_penalty: i128,
) -> Result<i128, SLAError> {
    let config: PenaltyEscalationConfig = match env.storage().instance().get(&PENALTY_ESC_KEY) {
        Some(c) => c,
        None => return Ok(base_penalty),
    };

    if !config.enabled {
        return Ok(base_penalty);
    }

    // Get or create violation tracker
    let tracker_key = Symbol::new(env, &format!("VT_{:?}", outage_id.to_buffer()));
    let mut tracker: ViolationTracker = env
        .storage()
        .instance()
        .get(&tracker_key)
        .unwrap_or(ViolationTracker {
            consecutive_violations: 0,
            last_violation_at: 0,
            escalated: false,
        });

    // Update tracker
    tracker.consecutive_violations = tracker.consecutive_violations.saturating_add(1);
    tracker.last_violation_at = env.ledger().timestamp();

    // Check if escalation should trigger
    if tracker.consecutive_violations >= config.trigger_count {
        tracker.escalated = true;
    }

    env.storage().instance().set(&tracker_key, &tracker);

    // Apply escalation multiplier if triggered
    if tracker.escalated {
        let escalated = base_penalty
            .saturating_mul(config.escalation_multiplier_bps as i128)
            .div_euclid(100);
        Ok(escalated)
    } else {
        Ok(base_penalty)
    }
}

/// Reset violation tracker for an outage (called after SLA met).
pub fn reset_violation_tracker(env: &Env, outage_id: &Symbol) {
    let tracker_key = Symbol::new(env, &format!("VT_{:?}", outage_id.to_buffer()));
    env.storage().instance().remove(&tracker_key);
}

/// Get the violation tracker for an outage.
pub fn get_violation_tracker(
    env: &Env,
    outage_id: &Symbol,
) -> Result<Option<ViolationTracker>, SLAError> {
    let tracker_key = Symbol::new(env, &format!("VT_{:?}", outage_id.to_buffer()));
    Ok(env.storage().instance().get(&tracker_key))
}

/// Get the penalty escalation configuration.
pub fn get_penalty_escalation_config(
    env: &Env,
) -> Result<PenaltyEscalationConfig, SLAError> {
    Ok(env
        .storage()
        .instance()
        .get(&PENALTY_ESC_KEY)
        .unwrap_or(PenaltyEscalationConfig {
            trigger_count: 3,
            escalation_multiplier_bps: 150,
            enabled: false,
        }))
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
