//! Dispute resolution: filing, escalation, arbitration, and settlement.
//!
//! Self-contained (no `crate::` dependency, matching the convention used by
//! sibling files such as `dispute_appeal.rs`) — this file is not yet wired
//! into `lib.rs` via `mod dispute;`, so it defines its own storage keys,
//! admin key, and error type rather than depending on `SLAError`/`ADMIN_KEY`
//! from the top-level contract.

use soroban_sdk::{contracterror, symbol_short, Address, BytesN, Env, Symbol};

// -----------------------------------------------------------------------
// Storage keys
// -----------------------------------------------------------------------
const DISPUTES_KEY: Symbol = symbol_short!("DISP");
const ADMIN_KEY: Symbol = symbol_short!("ADMIN");
/// Issue #722: per-arbitrator accumulated fee balance across all disputes.
const ARB_FEES_KEY: Symbol = symbol_short!("ARBFEE");
/// Issue #720: settlement payout record per dispute.
const PAYOUTS_KEY: Symbol = symbol_short!("PAYOUT");

// -----------------------------------------------------------------------
// Events
// -----------------------------------------------------------------------
const EVENT_DISPUTE_OPENED: Symbol = symbol_short!("disp_op");
const EVENT_DISPUTE_ESCALATED: Symbol = symbol_short!("disp_es");
const EVENT_DISPUTE_RESOLVED: Symbol = symbol_short!("disp_rv");
const EVENT_ARBITRATOR_VOTED: Symbol = symbol_short!("disp_vt");
const EVENT_SETTLEMENT_PAYOUT: Symbol = symbol_short!("disp_pay");
const EVENT_VERSION: Symbol = symbol_short!("v1");

// -----------------------------------------------------------------------
// Config
// -----------------------------------------------------------------------

/// Issue #723: hard cap on simultaneously active (Open / UnderReview /
/// Escalated) disputes across the contract, to bound storage growth.
pub const MAX_ACTIVE_DISPUTES: u32 = 50;

/// Issue #722: percentage of the dispute bond (in basis points, so 500 =
/// 5.00%) paid out to arbitrators who voted, split evenly among them.
const ARBITRATOR_FEE_BPS: i128 = 500;
const BPS_DENOMINATOR: i128 = 10_000;

/// Issue #721 / #716 (arbitration window timelock — tracked here as a
/// field so this issue's getter can report time remaining; the actual
/// timelock *enforcement* is a separate, not-yet-implemented concern):
/// default window from filing until the arbitration deadline.
const DEFAULT_ARBITRATION_WINDOW_SECS: u64 = 7 * 24 * 60 * 60;

/// All-zero sentinel for `Dispute::evidence_hash` meaning "not set yet" —
/// see the field's doc comment for why this isn't `Option<BytesN<32>>`.
fn no_evidence_hash(env: &Env) -> BytesN<32> {
    BytesN::from_array(env, &[0u8; 32])
}

// -----------------------------------------------------------------------
// Errors
// -----------------------------------------------------------------------
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum DisputeError {
    NotInitialized = 1,
    Unauthorized = 2,
    ConfigNotFound = 3,
    InvalidTransition = 4,
    /// Issue #723.
    MaxActiveDisputesReached = 5,
    /// Issue #722: vote cast against a dispute that is no longer active.
    DisputeNotActive = 6,
}

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

impl DisputeStatus {
    /// Whether a dispute in this status counts against `MAX_ACTIVE_DISPUTES`
    /// and can still accept arbitrator votes.
    fn is_active(&self) -> bool {
        matches!(
            self,
            DisputeStatus::Open | DisputeStatus::UnderReview | DisputeStatus::Escalated
        )
    }
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
#[derive(Clone)]
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
    /// Issue #722 / #720: bond posted by the filer, subject to the
    /// arbitrator fee split and customer payout on settlement.
    pub bond_amount: i128,
    /// Issue #721: hash of off-chain evidence for this dispute. All-zero
    /// (`NO_EVIDENCE_HASH`) until updated (the update helper itself is a
    /// separate concern from this issue's getter). `Option<BytesN<32>>`
    /// doesn't derive `#[contracttype]` cleanly in this soroban-sdk
    /// version, hence the sentinel value instead of `None`.
    pub evidence_hash: BytesN<32>,
    /// Issue #722 / #721: arbitrators who have cast a vote on this dispute.
    pub arbitrator_votes: soroban_sdk::Vec<Address>,
    /// Issue #721 / #716: ledger timestamp after which the arbitration
    /// window closes.
    pub arbitration_window_deadline: u64,
}

/// Issue #721: complete dispute status view for off-chain dashboards.
#[soroban_sdk::contracttype]
pub struct DisputeDetails {
    pub dispute_id: Symbol,
    pub status: DisputeStatus,
    pub escalation_level: EscalationLevel,
    /// `NO_EVIDENCE_HASH` (all-zero) if none has been recorded yet.
    pub evidence_hash: BytesN<32>,
    pub arbitrator_votes: soroban_sdk::Vec<Address>,
    pub bond_amount: i128,
    /// Seconds remaining until `arbitration_window_deadline`, or 0 if the
    /// window has already closed.
    pub time_remaining_secs: u64,
}

/// Issue #720: settlement payout record, computed automatically when a
/// dispute is resolved as upheld.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SettlementPayout {
    pub dispute_id: Symbol,
    pub upheld: bool,
    /// Portion of the bond released back to the customer (bond minus the
    /// arbitrator fee share when upheld; the full bond when not upheld —
    /// see `resolve_dispute`).
    pub customer_payout: i128,
    /// Issue #722: total arbitrator fee (5% of bond) when upheld, 0
    /// otherwise.
    pub arbitrator_fee_total: i128,
    /// `arbitrator_fee_total` divided evenly across the arbitrators who
    /// voted before settlement; 0 if no arbitrators voted.
    pub per_arbitrator_share: i128,
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
/// - `bond_amount`: Bond posted by the filer (issue #722 / #720).
///
/// # Events
/// - `disp_op`: Emitted when dispute is opened.
///
/// # Errors
/// Returns `MaxActiveDisputesReached` (issue #723) if `MAX_ACTIVE_DISPUTES`
/// active disputes already exist.
pub fn open_dispute(
    env: &Env,
    caller: &Address,
    dispute_id: Symbol,
    outage_id: Symbol,
    reason: soroban_sdk::String,
    bond_amount: i128,
) -> Result<(), DisputeError> {
    let now = env.ledger().timestamp();

    let mut disputes = load_disputes(env);

    // Issue #723: reject new filings once the active-dispute cap is hit.
    if count_active_disputes(&disputes) >= MAX_ACTIVE_DISPUTES {
        return Err(DisputeError::MaxActiveDisputesReached);
    }

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
        bond_amount,
        evidence_hash: no_evidence_hash(env),
        arbitrator_votes: soroban_sdk::Vec::new(env),
        arbitration_window_deadline: now + DEFAULT_ARBITRATION_WINDOW_SECS,
    };

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
) -> Result<(), DisputeError> {
    require_admin(env, caller)?;

    let mut disputes = load_disputes(env);

    let mut dispute = disputes
        .get(dispute_id.clone())
        .ok_or(DisputeError::ConfigNotFound)?;

    // Can only escalate open or under review disputes
    match dispute.status {
        DisputeStatus::Open | DisputeStatus::UnderReview => {}
        _ => return Err(DisputeError::InvalidTransition),
    }

    // Escalate to next level
    dispute.escalation_level = match dispute.escalation_level {
        EscalationLevel::L1Support => EscalationLevel::L2Engineering,
        EscalationLevel::L2Engineering => EscalationLevel::L3Management,
        EscalationLevel::L3Management => return Err(DisputeError::InvalidTransition),
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

/// Issue #722: cast an arbitrator's vote on an active dispute.
///
/// Any arbitrator who votes before the dispute is settled shares in the
/// arbitrator fee reward on resolution. Voting twice is a no-op (the
/// arbitrator is not double-counted).
///
/// # Events
/// - `disp_vt`: Emitted when an arbitrator casts a vote.
///
/// # Errors
/// Returns `DisputeNotActive` if the dispute has already been resolved or
/// dismissed.
pub fn cast_arbitrator_vote(
    env: &Env,
    arbitrator: &Address,
    dispute_id: Symbol,
) -> Result<(), DisputeError> {
    arbitrator.require_auth();

    let mut disputes = load_disputes(env);
    let mut dispute = disputes
        .get(dispute_id.clone())
        .ok_or(DisputeError::ConfigNotFound)?;

    if !dispute.status.is_active() {
        return Err(DisputeError::DisputeNotActive);
    }

    if !dispute.arbitrator_votes.contains(arbitrator) {
        dispute.arbitrator_votes.push_back(arbitrator.clone());
    }

    disputes.set(dispute_id.clone(), dispute);
    env.storage().instance().set(&DISPUTES_KEY, &disputes);

    env.events().publish(
        (EVENT_ARBITRATOR_VOTED, EVENT_VERSION, arbitrator.clone()),
        dispute_id,
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
/// - `upheld`: Whether the dispute was upheld in the filer's favor (issue
///   #720). When `true`, this automatically computes and records the
///   settlement payout: the bond minus the arbitrator fee (issue #722)
///   released to the customer, and the arbitrator fee split evenly among
///   arbitrators who voted before this call. When `false`, the full bond
///   is recorded as returned to the customer and no arbitrator fee is
///   paid.
///
/// # Events
/// - `disp_rv`: Emitted when dispute is resolved.
/// - `disp_pay`: Emitted with the computed settlement payout.
///
/// # Note
/// This records the payout as an on-chain accounting entry (queryable via
/// `get_dispute_details`/the `PAYOUTS_KEY`/`ARB_FEES_KEY` maps in this
/// file) rather than performing a cross-contract token transfer — this
/// contract has no token-client wiring anywhere in the codebase yet, so
/// actually moving funds is left to a follow-up change that adds one.
pub fn resolve_dispute(
    env: &Env,
    caller: &Address,
    dispute_id: Symbol,
    resolution: soroban_sdk::String,
    upheld: bool,
) -> Result<SettlementPayout, DisputeError> {
    require_admin(env, caller)?;

    let mut disputes = load_disputes(env);

    let mut dispute = disputes
        .get(dispute_id.clone())
        .ok_or(DisputeError::ConfigNotFound)?;

    // Can only resolve open, under review, or escalated disputes
    if !dispute.status.is_active() {
        return Err(DisputeError::InvalidTransition);
    }

    dispute.status = DisputeStatus::Resolved;
    dispute.resolution = Some(resolution);
    dispute.resolved_by = Some(caller.clone());
    dispute.resolved_at = Some(env.ledger().timestamp());

    // Issue #722 / #720: compute the settlement payout before votes are
    // locked in by the status change above.
    let voter_count = dispute.arbitrator_votes.len() as i128;
    let arbitrator_fee_total = if upheld && voter_count > 0 {
        dispute.bond_amount * ARBITRATOR_FEE_BPS / BPS_DENOMINATOR
    } else {
        0
    };
    let per_arbitrator_share = if voter_count > 0 {
        arbitrator_fee_total / voter_count
    } else {
        0
    };
    let customer_payout = dispute.bond_amount - arbitrator_fee_total;

    let payout = SettlementPayout {
        dispute_id: dispute_id.clone(),
        upheld,
        customer_payout,
        arbitrator_fee_total,
        per_arbitrator_share,
    };

    if per_arbitrator_share > 0 {
        let mut arb_fees = load_arbitrator_fees(env);
        for arbitrator in dispute.arbitrator_votes.iter() {
            let owed = arb_fees.get(arbitrator.clone()).unwrap_or(0);
            arb_fees.set(arbitrator, owed + per_arbitrator_share);
        }
        env.storage().instance().set(&ARB_FEES_KEY, &arb_fees);
    }

    let mut payouts = load_payouts(env);
    payouts.set(dispute_id.clone(), payout.clone());
    env.storage().instance().set(&PAYOUTS_KEY, &payouts);

    disputes.set(dispute_id.clone(), dispute);
    env.storage().instance().set(&DISPUTES_KEY, &disputes);

    env.events().publish(
        (EVENT_DISPUTE_RESOLVED, EVENT_VERSION, caller),
        (dispute_id.clone(), symbol_short!("resolve")),
    );
    env.events()
        .publish((EVENT_SETTLEMENT_PAYOUT, EVENT_VERSION), payout.clone());

    Ok(payout)
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
) -> Result<(), DisputeError> {
    require_admin(env, caller)?;

    let mut disputes = load_disputes(env);

    let mut dispute = disputes
        .get(dispute_id.clone())
        .ok_or(DisputeError::ConfigNotFound)?;

    dispute.status = DisputeStatus::Dismissed;
    dispute.resolved_by = Some(caller.clone());
    dispute.resolved_at = Some(env.ledger().timestamp());

    disputes.set(dispute_id.clone(), dispute);
    env.storage().instance().set(&DISPUTES_KEY, &disputes);

    Ok(())
}

/// Get a dispute by ID.
pub fn get_dispute(env: &Env, dispute_id: Symbol) -> Result<Option<Dispute>, DisputeError> {
    let disputes = load_disputes(env);
    Ok(disputes.get(dispute_id))
}

/// Issue #721: complete dispute status, for off-chain dashboards that need
/// evidence links, vote tallies, and the arbitration deadline in one call.
pub fn get_dispute_details(env: &Env, dispute_id: Symbol) -> Result<DisputeDetails, DisputeError> {
    let disputes = load_disputes(env);
    let dispute = disputes
        .get(dispute_id.clone())
        .ok_or(DisputeError::ConfigNotFound)?;

    let now = env.ledger().timestamp();
    let time_remaining_secs = dispute.arbitration_window_deadline.saturating_sub(now);

    Ok(DisputeDetails {
        dispute_id,
        status: dispute.status,
        escalation_level: dispute.escalation_level,
        evidence_hash: dispute.evidence_hash,
        arbitrator_votes: dispute.arbitrator_votes,
        bond_amount: dispute.bond_amount,
        time_remaining_secs,
    })
}

/// Issue #720: look up the recorded settlement payout for a resolved
/// dispute, if one has been computed.
pub fn get_settlement_payout(
    env: &Env,
    dispute_id: Symbol,
) -> Result<Option<SettlementPayout>, DisputeError> {
    let payouts = load_payouts(env);
    Ok(payouts.get(dispute_id))
}

/// Issue #722: the total fee balance an arbitrator has accrued across all
/// resolved disputes they voted on.
pub fn get_arbitrator_fee_balance(env: &Env, arbitrator: Address) -> i128 {
    load_arbitrator_fees(env).get(arbitrator).unwrap_or(0)
}

/// Get all disputes.
pub fn list_disputes(env: &Env) -> Result<soroban_sdk::Vec<Dispute>, DisputeError> {
    let disputes: soroban_sdk::Map<Symbol, Dispute> =
        match env.storage().instance().get(&DISPUTES_KEY) {
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
) -> Result<soroban_sdk::Vec<Dispute>, DisputeError> {
    let disputes: soroban_sdk::Map<Symbol, Dispute> =
        match env.storage().instance().get(&DISPUTES_KEY) {
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

/// Issue #723: number of currently active (Open / UnderReview / Escalated)
/// disputes.
fn count_active_disputes(disputes: &soroban_sdk::Map<Symbol, Dispute>) -> u32 {
    let mut count: u32 = 0;
    for (_, dispute) in disputes.iter() {
        if dispute.status.is_active() {
            count += 1;
        }
    }
    count
}

fn load_disputes(env: &Env) -> soroban_sdk::Map<Symbol, Dispute> {
    env.storage()
        .instance()
        .get(&DISPUTES_KEY)
        .unwrap_or_else(|| soroban_sdk::Map::new(env))
}

fn load_arbitrator_fees(env: &Env) -> soroban_sdk::Map<Address, i128> {
    env.storage()
        .instance()
        .get(&ARB_FEES_KEY)
        .unwrap_or_else(|| soroban_sdk::Map::new(env))
}

fn load_payouts(env: &Env) -> soroban_sdk::Map<Symbol, SettlementPayout> {
    env.storage()
        .instance()
        .get(&PAYOUTS_KEY)
        .unwrap_or_else(|| soroban_sdk::Map::new(env))
}

/// Helper to verify admin role.
fn require_admin(env: &Env, caller: &Address) -> Result<(), DisputeError> {
    let admin: Address = env
        .storage()
        .instance()
        .get(&ADMIN_KEY)
        .ok_or(DisputeError::NotInitialized)?;
    if *caller != admin {
        return Err(DisputeError::Unauthorized);
    }
    Ok(())
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------
#[cfg(test)]
mod tests {
    extern crate alloc;
    use alloc::format;

    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger};

    fn setup(env: &Env) -> (Address, Address) {
        let admin = Address::generate(env);
        let filer = Address::generate(env);
        env.storage().instance().set(&ADMIN_KEY, &admin);
        (admin, filer)
    }

    fn reason(env: &Env) -> soroban_sdk::String {
        soroban_sdk::String::from_str(env, "r")
    }

    #[test]
    fn test_max_active_disputes_cap_rejects_51st_dispute() {
        let env = Env::default();
        let (_, filer) = setup(&env);

        for i in 0..MAX_ACTIVE_DISPUTES {
            let id = Symbol::new(&env, &format!("d{}", i));
            let outage = Symbol::new(&env, "outage");
            open_dispute(&env, &filer, id, outage, reason(&env), 1_000).unwrap();
        }

        let one_too_many = Symbol::new(&env, "d_overflow");
        let outage = Symbol::new(&env, "outage");
        let result = open_dispute(&env, &filer, one_too_many, outage, reason(&env), 1_000);
        assert_eq!(result, Err(DisputeError::MaxActiveDisputesReached));
    }

    #[test]
    fn test_settling_a_dispute_frees_an_active_slot() {
        let env = Env::default();
        let (admin, filer) = setup(&env);

        for i in 0..MAX_ACTIVE_DISPUTES {
            let id = Symbol::new(&env, &format!("d{}", i));
            let outage = Symbol::new(&env, "outage");
            open_dispute(&env, &filer, id, outage, reason(&env), 1_000).unwrap();
        }

        let first_id = Symbol::new(&env, "d0");
        resolve_dispute(&env, &admin, first_id, reason(&env), false).unwrap();

        let new_id = Symbol::new(&env, "d_after_free");
        let outage = Symbol::new(&env, "outage");
        assert!(open_dispute(&env, &filer, new_id, outage, reason(&env), 1_000).is_ok());
    }

    #[test]
    fn test_arbitrator_fee_split_on_upheld_settlement() {
        let env = Env::default();
        env.mock_all_auths();
        let (admin, filer) = setup(&env);
        let arb1 = Address::generate(&env);
        let arb2 = Address::generate(&env);

        let id = Symbol::new(&env, "d1");
        let outage = Symbol::new(&env, "outage");
        open_dispute(&env, &filer, id.clone(), outage, reason(&env), 1_000).unwrap();

        cast_arbitrator_vote(&env, &arb1, id.clone()).unwrap();
        cast_arbitrator_vote(&env, &arb2, id.clone()).unwrap();

        let payout = resolve_dispute(&env, &admin, id, reason(&env), true).unwrap();

        // 5% of 1000 = 50, split between 2 arbitrators = 25 each.
        assert_eq!(payout.arbitrator_fee_total, 50);
        assert_eq!(payout.per_arbitrator_share, 25);
        assert_eq!(payout.customer_payout, 950);
        assert_eq!(get_arbitrator_fee_balance(&env, arb1), 25);
        assert_eq!(get_arbitrator_fee_balance(&env, arb2), 25);
    }

    #[test]
    fn test_no_arbitrator_fee_when_not_upheld() {
        let env = Env::default();
        env.mock_all_auths();
        let (admin, filer) = setup(&env);
        let arb1 = Address::generate(&env);

        let id = Symbol::new(&env, "d1");
        let outage = Symbol::new(&env, "outage");
        open_dispute(&env, &filer, id.clone(), outage, reason(&env), 1_000).unwrap();
        cast_arbitrator_vote(&env, &arb1, id.clone()).unwrap();

        let payout = resolve_dispute(&env, &admin, id, reason(&env), false).unwrap();

        assert_eq!(payout.arbitrator_fee_total, 0);
        assert_eq!(payout.customer_payout, 1_000);
        assert_eq!(get_arbitrator_fee_balance(&env, arb1), 0);
    }

    #[test]
    fn test_vote_rejected_once_dispute_resolved() {
        let env = Env::default();
        env.mock_all_auths();
        let (admin, filer) = setup(&env);
        let arb1 = Address::generate(&env);

        let id = Symbol::new(&env, "d1");
        let outage = Symbol::new(&env, "outage");
        open_dispute(&env, &filer, id.clone(), outage, reason(&env), 1_000).unwrap();

        resolve_dispute(&env, &admin, id.clone(), reason(&env), false).unwrap();

        let result = cast_arbitrator_vote(&env, &arb1, id);
        assert_eq!(result, Err(DisputeError::DisputeNotActive));
    }

    #[test]
    fn test_get_dispute_details_reports_full_summary() {
        let env = Env::default();
        env.mock_all_auths();
        let (_, filer) = setup(&env);
        let arb1 = Address::generate(&env);

        env.ledger().set_timestamp(1_000);
        let id = Symbol::new(&env, "d1");
        let outage = Symbol::new(&env, "outage");
        open_dispute(&env, &filer, id.clone(), outage, reason(&env), 2_000).unwrap();
        cast_arbitrator_vote(&env, &arb1, id.clone()).unwrap();

        let details = get_dispute_details(&env, id).unwrap();
        assert_eq!(details.status, DisputeStatus::Open);
        assert_eq!(details.bond_amount, 2_000);
        assert_eq!(details.arbitrator_votes.len(), 1);
        assert_eq!(details.evidence_hash, no_evidence_hash(&env));
        assert_eq!(details.time_remaining_secs, DEFAULT_ARBITRATION_WINDOW_SECS);
    }

    #[test]
    fn test_get_dispute_details_time_remaining_after_window_elapses() {
        let env = Env::default();
        let (_, filer) = setup(&env);

        env.ledger().set_timestamp(1_000);
        let id = Symbol::new(&env, "d1");
        let outage = Symbol::new(&env, "outage");
        open_dispute(&env, &filer, id.clone(), outage, reason(&env), 1_000).unwrap();

        env.ledger()
            .set_timestamp(1_000 + DEFAULT_ARBITRATION_WINDOW_SECS + 500);
        let details = get_dispute_details(&env, id).unwrap();
        assert_eq!(details.time_remaining_secs, 0);
    }
}
