#![no_std]

use soroban_sdk::{symbol_short, Address, Env, Symbol};

use crate::{SLAError, ADMIN_KEY};

// -----------------------------------------------------------------------
// Storage keys
// -----------------------------------------------------------------------
const DISPUTES_KEY: Symbol = symbol_short!("DISP");

// -----------------------------------------------------------------------
// Events
// -----------------------------------------------------------------------
const EVENT_DISPUTE_OPENED: Symbol = symbol_short!("disp_op");
const EVENT_DISPUTE_ESCALATED: Symbol = symbol_short!("disp_es");
const EVENT_DISPUTE_RESOLVED: Symbol = symbol_short!("disp_rv");
const EVENT_VERSION: Symbol = symbol_short!("v1");

// -----------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------

/// Dispute status.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DisputeStatus {
    Open,
    UnderReview,
    Escalated,
    Resolved,
    Dismissed,
}

/// Dispute escalation level.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EscalationLevel {
    L1Support,
    L2Engineering,
    L3Management,
}

/// A dispute record.
#[soroban_sdk::contracttype]
pub struct Dispute {
    /// Unique dispute identifier.
    pub dispute_id: Symbol,
    /// Outage ID being disputed.
    pub outage_id: Symbol,
    /// Who opened the dispute.
    pub opened_by: Address,
    /// When the dispute was opened.
    pub opened_at: u64,
    /// Current status.
    pub status: DisputeStatus,
    /// Current escalation level.
    pub escalation_level: EscalationLevel,
    /// Reason for the dispute.
    pub reason: soroban_sdk::String,
    /// Resolution notes (if resolved).
    pub resolution: Option<soroban_sdk::String>,
    /// Who resolved the dispute.
    pub resolved_by: Option<Address>,
    /// When the dispute was resolved.
    pub resolved_at: Option<u64>,
}

// -----------------------------------------------------------------------
// Functions
// -----------------------------------------------------------------------

/// Open a new dispute for an SLA calculation.
///
/// # Arguments
/// - `caller`: Address opening the dispute.
/// - `dispute_id`: Unique identifier for this dispute.
/// - `outage_id`: The outage ID being disputed.
/// - `reason`: Explanation of why the calculation is disputed.
///
/// # Events
/// - `disp_op`: Emitted when dispute is opened.
pub fn open_dispute(
    env: &Env,
    caller: &Address,
    dispute_id: Symbol,
    outage_id: Symbol,
    reason: soroban_sdk::String,
) -> Result<(), SLAError> {
    let now = env.ledger().timestamp();

    let dispute = Dispute {
        dispute_id: dispute_id.clone(),
        outage_id,
        opened_by: caller.clone(),
        opened_at: now,
        status: DisputeStatus::Open,
        escalation_level: EscalationLevel::L1Support,
        reason,
        resolution: None,
        resolved_by: None,
        resolved_at: None,
    };

    let mut disputes: soroban_sdk::Map<Symbol, Dispute> = env
        .storage()
        .instance()
        .get(&DISPUTES_KEY)
        .unwrap_or_else(|| soroban_sdk::Map::new(env));

    disputes.set(dispute_id.clone(), dispute);
    env.storage().instance().set(&DISPUTES_KEY, &disputes);

    env.events().publish(
        (EVENT_DISPUTE_OPENED, EVENT_VERSION, caller),
        (dispute_id, symbol_short!("open")),
    );

    Ok(())
}

/// Escalate a dispute to the next level.
///
/// Only admin can escalate disputes.
///
/// # Arguments
/// - `caller`: Must be admin.
/// - `dispute_id`: The dispute to escalate.
///
/// # Events
/// - `disp_es`: Emitted when dispute is escalated.
///
/// # Errors
/// Returns `Unauthorized` if caller is not admin, or if dispute is already
/// at maximum escalation level.
pub fn escalate_dispute(
    env: &Env,
    caller: &Address,
    dispute_id: Symbol,
) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    let mut disputes: soroban_sdk::Map<Symbol, Dispute> = env
        .storage()
        .instance()
        .get(&DISPUTES_KEY)
        .ok_or(SLAError::NotInitialized)?;

    let mut dispute = disputes
        .get(dispute_id.clone())
        .ok_or(SLAError::ConfigNotFound)?;

    // Can only escalate open or under review disputes
    match dispute.status {
        DisputeStatus::Open | DisputeStatus::UnderReview => {}
        _ => return Err(SLAError::InvalidThreshold),
    }

    // Escalate to next level
    dispute.escalation_level = match dispute.escalation_level {
        EscalationLevel::L1Support => EscalationLevel::L2Engineering,
        EscalationLevel::L2Engineering => EscalationLevel::L3Management,
        EscalationLevel::L3Management => return Err(SLAError::InvalidThreshold),
    };

    dispute.status = DisputeStatus::Escalated;

    disputes.set(dispute_id.clone(), dispute);
    env.storage().instance().set(&DISPUTES_KEY, &disputes);

    env.events().publish(
        (EVENT_DISPUTE_ESCALATED, EVENT_VERSION, caller),
        (dispute_id, symbol_short!("escal")),
    );

    Ok(())
}

/// Resolve a dispute.
///
/// Only admin can resolve disputes.
///
/// # Arguments
/// - `caller`: Must be admin.
/// - `dispute_id`: The dispute to resolve.
/// - `resolution`: Resolution notes.
///
/// # Events
/// - `disp_rv`: Emitted when dispute is resolved.
pub fn resolve_dispute(
    env: &Env,
    caller: &Address,
    dispute_id: Symbol,
    resolution: soroban_sdk::String,
) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    let mut disputes: soroban_sdk::Map<Symbol, Dispute> = env
        .storage()
        .instance()
        .get(&DISPUTES_KEY)
        .ok_or(SLAError::NotInitialized)?;

    let mut dispute = disputes
        .get(dispute_id.clone())
        .ok_or(SLAError::ConfigNotFound)?;

    // Can only resolve open, under review, or escalated disputes
    match dispute.status {
        DisputeStatus::Open | DisputeStatus::UnderReview | DisputeStatus::Escalated => {}
        _ => return Err(SLAError::InvalidThreshold),
    }

    dispute.status = DisputeStatus::Resolved;
    dispute.resolution = Some(resolution);
    dispute.resolved_by = Some(caller.clone());
    dispute.resolved_at = Some(env.ledger().timestamp());

    disputes.set(dispute_id.clone(), dispute);
    env.storage().instance().set(&DISPUTES_KEY, &disputes);

    env.events().publish(
        (EVENT_DISPUTE_RESOLVED, EVENT_VERSION, caller),
        (dispute_id, symbol_short!("resolve")),
    );

    Ok(())
}

/// Dismiss a dispute (admin only).
///
/// # Arguments
/// - `caller`: Must be admin.
/// - `dispute_id`: The dispute to dismiss.
pub fn dismiss_dispute(
    env: &Env,
    caller: &Address,
    dispute_id: Symbol,
) -> Result<(), SLAError> {
    require_admin(env, caller)?;

    let mut disputes: soroban_sdk::Map<Symbol, Dispute> = env
        .storage()
        .instance()
        .get(&DISPUTES_KEY)
        .ok_or(SLAError::NotInitialized)?;

    let mut dispute = disputes
        .get(dispute_id.clone())
        .ok_or(SLAError::ConfigNotFound)?;

    dispute.status = DisputeStatus::Dismissed;
    dispute.resolved_by = Some(caller.clone());
    dispute.resolved_at = Some(env.ledger().timestamp());

    disputes.set(dispute_id.clone(), dispute);
    env.storage().instance().set(&DISPUTES_KEY, &disputes);

    Ok(())
}

/// Get a dispute by ID.
pub fn get_dispute(
    env: &Env,
    dispute_id: Symbol,
) -> Result<Option<Dispute>, SLAError> {
    let disputes: soroban_sdk::Map<Symbol, Dispute> = env
        .storage()
        .instance()
        .get(&DISPUTES_KEY)
        .ok_or(SLAError::NotInitialized)?;

    Ok(disputes.get(dispute_id))
}

/// Get all disputes.
pub fn list_disputes(env: &Env) -> Result<soroban_sdk::Vec<Dispute>, SLAError> {
    let disputes: soroban_sdk::Map<Symbol, Dispute> = match env
        .storage()
        .instance()
        .get(&DISPUTES_KEY)
    {
        Some(d) => d,
        None => return Ok(soroban_sdk::Vec::new(env)),
    };

    let mut result = soroban_sdk::Vec::new(env);
    for (_, dispute) in disputes.iter() {
        result.push_back(dispute);
    }
    Ok(result)
}

/// Get disputes by status.
pub fn get_disputes_by_status(
    env: &Env,
    status: DisputeStatus,
) -> Result<soroban_sdk::Vec<Dispute>, SLAError> {
    let disputes: soroban_sdk::Map<Symbol, Dispute> = match env
        .storage()
        .instance()
        .get(&DISPUTES_KEY)
    {
        Some(d) => d,
        None => return Ok(soroban_sdk::Vec::new(env)),
    };

    let mut result = soroban_sdk::Vec::new(env);
    for (_, dispute) in disputes.iter() {
        if dispute.status == status {
            result.push_back(dispute);
        }
    }
    Ok(result)
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
