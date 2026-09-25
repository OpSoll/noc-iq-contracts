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
/// Issue #718: total value slashed from frivolous-dispute bonds, held for
/// the arbitration pool reserve.
const ARBITRATION_POOL_KEY: Symbol = symbol_short!("ARBPOOL");

// -----------------------------------------------------------------------
// Events
// -----------------------------------------------------------------------
const EVENT_DISPUTE_OPENED: Symbol = symbol_short!("disp_op");
const EVENT_DISPUTE_ESCALATED: Symbol = symbol_short!("disp_es");
const EVENT_DISPUTE_RESOLVED: Symbol = symbol_short!("disp_rv");
const EVENT_ARBITRATOR_VOTED: Symbol = symbol_short!("disp_vt");
const EVENT_SETTLEMENT_PAYOUT: Symbol = symbol_short!("disp_pay");
/// Issue #719.
const EVENT_EVIDENCE_ADDED: Symbol = symbol_short!("disp_ev");
/// Issue #717: emitted when 2-of-3 quorum finalizes a dispute automatically.
const EVENT_QUORUM_REACHED: Symbol = symbol_short!("disp_qr");
/// Issue #716: emitted when the arbitration window expires unresolved.
const EVENT_AUTO_RESOLVED: Symbol = symbol_short!("disp_ar");
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

/// Issue #716: window from filing until the arbitration deadline. After
/// this elapses without a resolution, `auto_resolve_expired_dispute` lets
/// anyone finalize the dispute in favor of the reporter.
const DEFAULT_ARBITRATION_WINDOW_SECS: u64 = 14 * 24 * 60 * 60;

/// Issue #717: size of the designated arbitrator panel and the quorum
/// (majority) of votes required to auto-finalize a dispute.
const ARBITRATOR_PANEL_SIZE: u32 = 3;
const ARBITRATOR_QUORUM: u32 = 2;

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
    /// Issue #719: evidence submitted by someone other than the dispute's
    /// original filer.
    NotDisputeFiler = 7,
    /// Issue #717: panel assignment that isn't exactly `ARBITRATOR_PANEL_SIZE`
    /// addresses, or a vote cast by an address not on the assigned panel.
    InvalidArbitratorPanel = 8,
    /// Issue #716: the arbitration window has not yet elapsed.
    ArbitrationWindowNotExpired = 9,
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

/// Issue #717: an arbitrator's decision on a dispute.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VoteDecision {
    /// The outage breach is valid — the dispute is upheld.
    UpholdOutage,
    /// The outage breach is invalid — the dispute is dismissed.
    DismissOutage,
}

/// Issue #717 / #718: a single arbitrator's recorded vote, including the
/// issue #718 "frivolous" tag used for bond-slashing eligibility.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArbitratorVote {
    pub arbitrator: Address,
    pub decision: VoteDecision,
    /// Issue #718: arbitrator's assessment that the dispute itself was
    /// filed in bad faith / without valid evidence.
    pub frivolous: bool,
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
    /// Issue #719: full history of evidence hashes submitted for this
    /// dispute, most recent last. `evidence_hash` always mirrors the last
    /// entry (or the all-zero sentinel when empty).
    pub evidence_history: soroban_sdk::Vec<BytesN<32>>,
    /// Issue #722 / #721: arbitrators who have cast a vote on this dispute.
    pub arbitrator_votes: soroban_sdk::Vec<Address>,
    /// Issue #717 / #718: detailed per-arbitrator decisions (upheld/
    /// dismissed + frivolous tag), one per entry in `arbitrator_votes`.
    pub votes_detail: soroban_sdk::Vec<ArbitratorVote>,
    /// Issue #717: the exactly-`ARBITRATOR_PANEL_SIZE` addresses designated
    /// to vote on this dispute. Empty until `assign_arbitrator_panel` is
    /// called, in which case (for backward compatibility with disputes
    /// filed before panel assignment existed) any arbitrator may still
    /// vote via `cast_arbitrator_vote`.
    pub designated_arbitrators: soroban_sdk::Vec<Address>,
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

/// Issue #715: formal SLA dispute filing protocol. Lets a customer open a
/// dispute against an outage record, locking a bond and attaching initial
/// evidence in a single call.
///
/// # Arguments
/// - `caller`: Address opening the dispute.
/// - `dispute_id`: Unique identifier for this dispute.
/// - `outage_id`: The outage ID being disputed — this dispute's existence
///   in `Open` status is this module's record of the outage now being
///   under dispute (this file has no cross-contract link to an outage
///   record to flip a status field on directly; see the module doc).
/// - `reason`: Explanation of why the calculation is disputed.
/// - `bond_amount`: Bond posted by the filer, locked in this contract's
///   accounting (issue #722 / #720 govern how it's later released/split).
/// - `evidence_hash`: Initial off-chain evidence hash for the dispute.
///   Pass `no_evidence_hash(env)` if none is available yet — more can be
///   appended later via `add_dispute_evidence` (issue #719).
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
    evidence_hash: BytesN<32>,
) -> Result<(), DisputeError> {
    let now = env.ledger().timestamp();

    let mut disputes = load_disputes(env);

    // Issue #723: reject new filings once the active-dispute cap is hit.
    if count_active_disputes(&disputes) >= MAX_ACTIVE_DISPUTES {
        return Err(DisputeError::MaxActiveDisputesReached);
    }

    let mut evidence_history = soroban_sdk::Vec::new(env);
    if evidence_hash != no_evidence_hash(env) {
        evidence_history.push_back(evidence_hash.clone());
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
        evidence_history: soroban_sdk::Vec::new(env),
        arbitrator_votes: soroban_sdk::Vec::new(env),
        votes_detail: soroban_sdk::Vec::new(env),
        designated_arbitrators: soroban_sdk::Vec::new(env),
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

/// Issue #719: append an updated evidence hash to an active dispute,
/// instead of requiring a whole new dispute instance for follow-up
/// evidence.
///
/// # Arguments
/// - `caller`: Must be the dispute's original filer.
/// - `dispute_id`: The dispute to update.
/// - `new_evidence_hash`: Hash of the newly-submitted off-chain evidence
///   (e.g. an IPFS CID digest).
///
/// # Events
/// - `disp_ev`: Emitted with the dispute ID and updated evidence count.
///
/// # Errors
/// Returns `NotDisputeFiler` if `caller` did not open the dispute, or
/// `DisputeNotActive` if it has already been resolved or dismissed.
pub fn add_dispute_evidence(
    env: &Env,
    caller: &Address,
    dispute_id: Symbol,
    new_evidence_hash: BytesN<32>,
) -> Result<(), DisputeError> {
    caller.require_auth();

    let mut disputes = load_disputes(env);
    let mut dispute = disputes
        .get(dispute_id.clone())
        .ok_or(DisputeError::ConfigNotFound)?;

    if dispute.opened_by != *caller {
        return Err(DisputeError::NotDisputeFiler);
    }
    if !dispute.status.is_active() {
        return Err(DisputeError::DisputeNotActive);
    }

    dispute
        .evidence_history
        .push_back(new_evidence_hash.clone());
    dispute.evidence_hash = new_evidence_hash;

    let history_len = dispute.evidence_history.len();
    disputes.set(dispute_id.clone(), dispute);
    env.storage().instance().set(&DISPUTES_KEY, &disputes);

    env.events().publish(
        (EVENT_EVIDENCE_ADDED, EVENT_VERSION, caller),
        (dispute_id, history_len),
    );

    Ok(())
}

/// Issue #717: designate the 3-member arbitrator panel for a dispute.
/// Admin only. Once assigned, only panel members can vote on this dispute.
///
/// # Errors
/// Returns `InvalidArbitratorPanel` if `panel` does not contain exactly
/// `ARBITRATOR_PANEL_SIZE` (3) addresses.
pub fn assign_arbitrator_panel(
    env: &Env,
    caller: &Address,
    dispute_id: Symbol,
    panel: soroban_sdk::Vec<Address>,
) -> Result<(), DisputeError> {
    require_admin(env, caller)?;

    if panel.len() != ARBITRATOR_PANEL_SIZE {
        return Err(DisputeError::InvalidArbitratorPanel);
    }

    let mut disputes = load_disputes(env);
    let mut dispute = disputes
        .get(dispute_id.clone())
        .ok_or(DisputeError::ConfigNotFound)?;

    if !dispute.status.is_active() {
        return Err(DisputeError::DisputeNotActive);
    }

    dispute.designated_arbitrators = panel;
    disputes.set(dispute_id.clone(), dispute);
    env.storage().instance().set(&DISPUTES_KEY, &disputes);

    Ok(())
}

/// Issue #722 / #717 / #718: cast an arbitrator's vote on an active
/// dispute.
///
/// Any arbitrator who votes before the dispute is settled shares in the
/// arbitrator fee reward on resolution (issue #722). Voting twice is a
/// no-op for the address (not double-counted), it does not update a
/// previously-recorded decision.
///
/// If a 3-member panel has been assigned via `assign_arbitrator_panel`,
/// only panel members may vote; otherwise any address may (matching the
/// original, pre-panel behavior).
///
/// Once `ARBITRATOR_QUORUM` (2 of 3) votes agree on the same
/// `VoteDecision`, the dispute is automatically finalized (issue #717) —
/// see `try_tally_quorum` for the upheld/slashing rules (issue #718) this
/// applies.
///
/// # Events
/// - `disp_vt`: Emitted when an arbitrator casts a vote.
/// - `disp_qr`, `disp_rv`, `disp_pay`: Emitted (via `settle_dispute`) if
///   this vote reaches quorum and auto-finalizes the dispute.
///
/// # Errors
/// Returns `DisputeNotActive` if the dispute has already been resolved or
/// dismissed, or `InvalidArbitratorPanel` if a panel is assigned and
/// `arbitrator` is not on it.
pub fn cast_arbitrator_vote(
    env: &Env,
    arbitrator: &Address,
    dispute_id: Symbol,
    decision: VoteDecision,
    frivolous: bool,
) -> Result<Option<SettlementPayout>, DisputeError> {
    arbitrator.require_auth();

    let mut disputes = load_disputes(env);
    let mut dispute = disputes
        .get(dispute_id.clone())
        .ok_or(DisputeError::ConfigNotFound)?;

    if !dispute.status.is_active() {
        return Err(DisputeError::DisputeNotActive);
    }

    if !dispute.designated_arbitrators.is_empty()
        && !dispute.designated_arbitrators.contains(arbitrator)
    {
        return Err(DisputeError::InvalidArbitratorPanel);
    }

    if !dispute.arbitrator_votes.contains(arbitrator) {
        dispute.arbitrator_votes.push_back(arbitrator.clone());
        dispute.votes_detail.push_back(ArbitratorVote {
            arbitrator: arbitrator.clone(),
            decision,
            frivolous,
        });
    }

    env.events().publish(
        (EVENT_ARBITRATOR_VOTED, EVENT_VERSION, arbitrator.clone()),
        dispute_id.clone(),
    );

    if let Some((upheld, slash_bond, resolution)) = try_tally_quorum(env, &dispute) {
        env.events()
            .publish((EVENT_QUORUM_REACHED, EVENT_VERSION), dispute_id.clone());
        let payout = settle_dispute(
            env,
            disputes,
            dispute_id,
            dispute,
            arbitrator.clone(),
            resolution,
            upheld,
            slash_bond,
        );
        return Ok(Some(payout));
    }

    disputes.set(dispute_id.clone(), dispute);
    env.storage().instance().set(&DISPUTES_KEY, &disputes);

    Ok(None)
}

/// Issue #717 / #718: tally `dispute.votes_detail` and, if quorum has been
/// reached, return `(upheld, slash_bond, resolution_note)` for
/// `settle_dispute`.
///
/// - `>= ARBITRATOR_QUORUM` `UpholdOutage` votes: dispute upheld.
/// - `>= ARBITRATOR_QUORUM` `DismissOutage` votes: dispute dismissed; if
///   *every* panel seat (`ARBITRATOR_PANEL_SIZE`) voted `DismissOutage`
///   with `frivolous: true`, the bond is slashed 100% to the arbitration
///   pool instead of refunded (issue #718).
fn try_tally_quorum(env: &Env, dispute: &Dispute) -> Option<(bool, bool, soroban_sdk::String)> {
    let mut uphold_count: u32 = 0;
    let mut dismiss_count: u32 = 0;
    let mut all_frivolous_dismiss = true;

    for vote in dispute.votes_detail.iter() {
        match vote.decision {
            VoteDecision::UpholdOutage => {
                uphold_count += 1;
                all_frivolous_dismiss = false;
            }
            VoteDecision::DismissOutage => {
                dismiss_count += 1;
                if !vote.frivolous {
                    all_frivolous_dismiss = false;
                }
            }
        }
    }

    if uphold_count >= ARBITRATOR_QUORUM {
        return Some((
            true,
            false,
            soroban_sdk::String::from_str(
                env,
                "Auto-resolved: arbitrator quorum upheld the outage breach",
            ),
        ));
    }

    if dismiss_count >= ARBITRATOR_QUORUM {
        let total_votes = uphold_count + dismiss_count;
        let slash = total_votes == ARBITRATOR_PANEL_SIZE && all_frivolous_dismiss;
        let resolution = if slash {
            soroban_sdk::String::from_str(
                env,
                "Auto-resolved: unanimous frivolous dismissal, bond slashed to arbitration pool",
            )
        } else {
            soroban_sdk::String::from_str(
                env,
                "Auto-resolved: arbitrator quorum dismissed the outage breach",
            )
        };
        return Some((false, slash, resolution));
    }

    None
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

    let disputes = load_disputes(env);

    let dispute = disputes
        .get(dispute_id.clone())
        .ok_or(DisputeError::ConfigNotFound)?;

    // Can only resolve open, under review, or escalated disputes
    if !dispute.status.is_active() {
        return Err(DisputeError::InvalidTransition);
    }

    Ok(settle_dispute(
        env,
        disputes,
        dispute_id,
        dispute,
        caller.clone(),
        resolution,
        upheld,
        false,
    ))
}

/// Issue #716: permissionlessly resolve a dispute in favor of the filer
/// once its arbitration window has elapsed without a manual or
/// quorum-driven resolution. Anyone may call this — it exists so a filer
/// is never stuck waiting on an admin or arbitrator panel that never
/// acts.
///
/// # Events
/// - `disp_ar`: Emitted when a dispute is auto-resolved this way.
/// - `disp_rv`, `disp_pay`: Emitted (via `settle_dispute`) as with any
///   other resolution.
///
/// # Errors
/// Returns `DisputeNotActive` if the dispute was already resolved or
/// dismissed, or `ArbitrationWindowNotExpired` if `arbitration_window_deadline`
/// has not yet passed.
pub fn auto_resolve_expired_dispute(
    env: &Env,
    dispute_id: Symbol,
) -> Result<SettlementPayout, DisputeError> {
    let disputes = load_disputes(env);

    let dispute = disputes
        .get(dispute_id.clone())
        .ok_or(DisputeError::ConfigNotFound)?;

    if !dispute.status.is_active() {
        return Err(DisputeError::DisputeNotActive);
    }

    if env.ledger().timestamp() <= dispute.arbitration_window_deadline {
        return Err(DisputeError::ArbitrationWindowNotExpired);
    }

    let resolved_by = dispute.opened_by.clone();
    let resolution = soroban_sdk::String::from_str(
        env,
        "Auto-resolved: arbitration window expired, defaulted in favor of filer",
    );

    env.events()
        .publish((EVENT_AUTO_RESOLVED, EVENT_VERSION), dispute_id.clone());

    Ok(settle_dispute(
        env,
        disputes,
        dispute_id,
        dispute,
        resolved_by,
        resolution,
        true,
        false,
    ))
}

/// Shared finalization path for `resolve_dispute`, `cast_arbitrator_vote`
/// (quorum auto-finalization, issue #717) and
/// `auto_resolve_expired_dispute` (issue #716).
///
/// Marks `dispute` resolved, computes and records the `SettlementPayout`,
/// and persists everything to storage.
///
/// - `slash_bond` (issue #718): when `true`, 100% of the bond is
///   transferred to the `ARBITRATION_POOL_KEY` reserve instead of being
///   split between the customer and voting arbitrators — used for the
///   unanimous frivolous-dismissal case.
/// - Otherwise, when `upheld`, the arbitrator fee (issue #722) is
///   deducted from the bond and split evenly among voting arbitrators,
///   with the remainder refunded to the customer; when not upheld, the
///   full bond is recorded as returned to the customer.
fn settle_dispute(
    env: &Env,
    mut disputes: soroban_sdk::Map<Symbol, Dispute>,
    dispute_id: Symbol,
    mut dispute: Dispute,
    resolved_by: Address,
    resolution: soroban_sdk::String,
    upheld: bool,
    slash_bond: bool,
) -> SettlementPayout {
    dispute.status = DisputeStatus::Resolved;
    dispute.resolution = Some(resolution);
    dispute.resolved_by = Some(resolved_by.clone());
    dispute.resolved_at = Some(env.ledger().timestamp());

    let voter_count = dispute.arbitrator_votes.len() as i128;

    let (customer_payout, arbitrator_fee_total, per_arbitrator_share, pool_amount) = if slash_bond {
        (0, 0, 0, dispute.bond_amount)
    } else {
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
        (
            customer_payout,
            arbitrator_fee_total,
            per_arbitrator_share,
            0,
        )
    };

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

    if pool_amount > 0 {
        let current_pool: i128 = env
            .storage()
            .instance()
            .get(&ARBITRATION_POOL_KEY)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&ARBITRATION_POOL_KEY, &(current_pool + pool_amount));
    }

    let mut payouts = load_payouts(env);
    payouts.set(dispute_id.clone(), payout.clone());
    env.storage().instance().set(&PAYOUTS_KEY, &payouts);

    disputes.set(dispute_id.clone(), dispute);
    env.storage().instance().set(&DISPUTES_KEY, &disputes);

    env.events().publish(
        (EVENT_DISPUTE_RESOLVED, EVENT_VERSION, resolved_by),
        (dispute_id.clone(), symbol_short!("resolve")),
    );
    env.events()
        .publish((EVENT_SETTLEMENT_PAYOUT, EVENT_VERSION), payout.clone());

    payout
}

/// Issue #718: get the current balance of the arbitration pool reserve
/// (funds slashed from bonds on unanimous frivolous dismissals).
pub fn get_arbitration_pool_balance(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&ARBITRATION_POOL_KEY)
        .unwrap_or(0)
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
            open_dispute(
                &env,
                &filer,
                id,
                outage,
                reason(&env),
                1_000,
                no_evidence_hash(&env),
            )
            .unwrap();
        }

        let one_too_many = Symbol::new(&env, "d_overflow");
        let outage = Symbol::new(&env, "outage");
        let result = open_dispute(
            &env,
            &filer,
            one_too_many,
            outage,
            reason(&env),
            1_000,
            no_evidence_hash(&env),
        );
        assert_eq!(result, Err(DisputeError::MaxActiveDisputesReached));
    }

    #[test]
    fn test_settling_a_dispute_frees_an_active_slot() {
        let env = Env::default();
        let (admin, filer) = setup(&env);

        for i in 0..MAX_ACTIVE_DISPUTES {
            let id = Symbol::new(&env, &format!("d{}", i));
            let outage = Symbol::new(&env, "outage");
            open_dispute(
                &env,
                &filer,
                id,
                outage,
                reason(&env),
                1_000,
                no_evidence_hash(&env),
            )
            .unwrap();
        }

        let first_id = Symbol::new(&env, "d0");
        resolve_dispute(&env, &admin, first_id, reason(&env), false).unwrap();

        let new_id = Symbol::new(&env, "d_after_free");
        let outage = Symbol::new(&env, "outage");
        assert!(open_dispute(
            &env,
            &filer,
            new_id,
            outage,
            reason(&env),
            1_000,
            no_evidence_hash(&env)
        )
        .is_ok());
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
        open_dispute(
            &env,
            &filer,
            id.clone(),
            outage,
            reason(&env),
            1_000,
            no_evidence_hash(&env),
        )
        .unwrap();

        // Split vote (1 uphold, 1 non-frivolous dismiss) so quorum auto-
        // finalization (issue #717) doesn't preempt this admin resolution.
        cast_arbitrator_vote(&env, &arb1, id.clone(), VoteDecision::UpholdOutage, false).unwrap();
        cast_arbitrator_vote(&env, &arb2, id.clone(), VoteDecision::DismissOutage, false).unwrap();

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
        cast_arbitrator_vote(&env, &arb1, id.clone(), VoteDecision::UpholdOutage, false).unwrap();

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
        open_dispute(
            &env,
            &filer,
            id.clone(),
            outage,
            reason(&env),
            1_000,
            no_evidence_hash(&env),
        )
        .unwrap();

        resolve_dispute(&env, &admin, id.clone(), reason(&env), false).unwrap();

        let result = cast_arbitrator_vote(&env, &arb1, id, VoteDecision::UpholdOutage, false);
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
        cast_arbitrator_vote(&env, &arb1, id.clone(), VoteDecision::UpholdOutage, false).unwrap();

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
        open_dispute(
            &env,
            &filer,
            id.clone(),
            outage,
            reason(&env),
            1_000,
            no_evidence_hash(&env),
        )
        .unwrap();

        env.ledger()
            .set_timestamp(1_000 + DEFAULT_ARBITRATION_WINDOW_SECS + 500);
        let details = get_dispute_details(&env, id).unwrap();
        assert_eq!(details.time_remaining_secs, 0);
    }

    fn evidence_hash(env: &Env, seed: u8) -> BytesN<32> {
        BytesN::from_array(env, &[seed; 32])
    }

    #[test]
    fn test_add_dispute_evidence_appends_history() {
        let env = Env::default();
        env.mock_all_auths();
        let (_, filer) = setup(&env);

        let id = Symbol::new(&env, "d1");
        let outage = Symbol::new(&env, "outage");
        open_dispute(&env, &filer, id.clone(), outage, reason(&env), 1_000).unwrap();

        add_dispute_evidence(&env, &filer, id.clone(), evidence_hash(&env, 1)).unwrap();
        add_dispute_evidence(&env, &filer, id.clone(), evidence_hash(&env, 2)).unwrap();

        let details = get_dispute_details(&env, id).unwrap();
        assert_eq!(details.evidence_hash, evidence_hash(&env, 2));
    }

    #[test]
    fn test_add_dispute_evidence_rejects_non_filer() {
        let env = Env::default();
        env.mock_all_auths();
        let (_, filer) = setup(&env);
        let stranger = Address::generate(&env);

        let id = Symbol::new(&env, "d1");
        let outage = Symbol::new(&env, "outage");
        open_dispute(&env, &filer, id.clone(), outage, reason(&env), 1_000).unwrap();

        let result = add_dispute_evidence(&env, &stranger, id, evidence_hash(&env, 1));
        assert_eq!(result, Err(DisputeError::NotDisputeFiler));
    }

    #[test]
    fn test_assign_arbitrator_panel_requires_exact_size() {
        let env = Env::default();
        env.mock_all_auths();
        let (admin, filer) = setup(&env);
        let arb1 = Address::generate(&env);
        let arb2 = Address::generate(&env);

        let id = Symbol::new(&env, "d1");
        let outage = Symbol::new(&env, "outage");
        open_dispute(&env, &filer, id.clone(), outage, reason(&env), 1_000).unwrap();

        let mut panel = soroban_sdk::Vec::new(&env);
        panel.push_back(arb1);
        panel.push_back(arb2);

        let result = assign_arbitrator_panel(&env, &admin, id, panel);
        assert_eq!(result, Err(DisputeError::InvalidArbitratorPanel));
    }

    #[test]
    fn test_cast_vote_rejected_for_non_panel_member() {
        let env = Env::default();
        env.mock_all_auths();
        let (admin, filer) = setup(&env);
        let arb1 = Address::generate(&env);
        let arb2 = Address::generate(&env);
        let arb3 = Address::generate(&env);
        let outsider = Address::generate(&env);

        let id = Symbol::new(&env, "d1");
        let outage = Symbol::new(&env, "outage");
        open_dispute(&env, &filer, id.clone(), outage, reason(&env), 1_000).unwrap();

        let mut panel = soroban_sdk::Vec::new(&env);
        panel.push_back(arb1);
        panel.push_back(arb2);
        panel.push_back(arb3);
        assign_arbitrator_panel(&env, &admin, id.clone(), panel).unwrap();

        let result = cast_arbitrator_vote(&env, &outsider, id, VoteDecision::UpholdOutage, false);
        assert_eq!(result, Err(DisputeError::InvalidArbitratorPanel));
    }

    #[test]
    fn test_quorum_uphold_auto_finalizes_and_pays_arbitrators() {
        let env = Env::default();
        env.mock_all_auths();
        let (admin, filer) = setup(&env);
        let arb1 = Address::generate(&env);
        let arb2 = Address::generate(&env);
        let arb3 = Address::generate(&env);

        let id = Symbol::new(&env, "d1");
        let outage = Symbol::new(&env, "outage");
        open_dispute(&env, &filer, id.clone(), outage, reason(&env), 1_000).unwrap();

        let mut panel = soroban_sdk::Vec::new(&env);
        panel.push_back(arb1.clone());
        panel.push_back(arb2.clone());
        panel.push_back(arb3.clone());
        assign_arbitrator_panel(&env, &admin, id.clone(), panel).unwrap();

        let result =
            cast_arbitrator_vote(&env, &arb1, id.clone(), VoteDecision::UpholdOutage, false)
                .unwrap();
        assert!(result.is_none());

        // Second matching vote reaches 2-of-3 quorum and auto-finalizes.
        let result =
            cast_arbitrator_vote(&env, &arb2, id.clone(), VoteDecision::UpholdOutage, false)
                .unwrap();
        let payout = result.expect("quorum should auto-finalize the dispute");
        assert!(payout.upheld);
        assert_eq!(payout.arbitrator_fee_total, 50);
        assert_eq!(payout.customer_payout, 950);

        let details = get_dispute_details(&env, id).unwrap();
        assert_eq!(details.status, DisputeStatus::Resolved);

        // Third arbitrator's vote is now rejected since the dispute is settled.
        let late_vote = cast_arbitrator_vote(
            &env,
            &arb3,
            Symbol::new(&env, "d1"),
            VoteDecision::UpholdOutage,
            false,
        );
        assert_eq!(late_vote, Err(DisputeError::DisputeNotActive));
    }

    #[test]
    fn test_quorum_unanimous_frivolous_dismissal_slashes_bond() {
        let env = Env::default();
        env.mock_all_auths();
        let (admin, filer) = setup(&env);
        let arb1 = Address::generate(&env);
        let arb2 = Address::generate(&env);
        let arb3 = Address::generate(&env);

        let id = Symbol::new(&env, "d1");
        let outage = Symbol::new(&env, "outage");
        open_dispute(&env, &filer, id.clone(), outage, reason(&env), 1_000).unwrap();

        let mut panel = soroban_sdk::Vec::new(&env);
        panel.push_back(arb1.clone());
        panel.push_back(arb2.clone());
        panel.push_back(arb3.clone());
        assign_arbitrator_panel(&env, &admin, id.clone(), panel).unwrap();

        let pool_before = get_arbitration_pool_balance(&env);

        cast_arbitrator_vote(&env, &arb1, id.clone(), VoteDecision::DismissOutage, true).unwrap();
        cast_arbitrator_vote(&env, &arb2, id.clone(), VoteDecision::DismissOutage, true).unwrap();
        let payout =
            cast_arbitrator_vote(&env, &arb3, id.clone(), VoteDecision::DismissOutage, true)
                .unwrap()
                .expect("unanimous frivolous dismissal should auto-finalize");

        assert!(!payout.upheld);
        assert_eq!(payout.customer_payout, 0);
        assert_eq!(payout.arbitrator_fee_total, 0);
        assert_eq!(get_arbitration_pool_balance(&env), pool_before + 1_000);
    }

    #[test]
    fn test_quorum_dismissal_not_unanimous_frivolous_refunds_customer() {
        let env = Env::default();
        env.mock_all_auths();
        let (admin, filer) = setup(&env);
        let arb1 = Address::generate(&env);
        let arb2 = Address::generate(&env);
        let arb3 = Address::generate(&env);

        let id = Symbol::new(&env, "d1");
        let outage = Symbol::new(&env, "outage");
        open_dispute(&env, &filer, id.clone(), outage, reason(&env), 1_000).unwrap();

        let mut panel = soroban_sdk::Vec::new(&env);
        panel.push_back(arb1.clone());
        panel.push_back(arb2.clone());
        panel.push_back(arb3.clone());
        assign_arbitrator_panel(&env, &admin, id.clone(), panel).unwrap();

        // Not unanimous frivolous: arb2 dismisses without the frivolous tag.
        let pool_before = get_arbitration_pool_balance(&env);
        cast_arbitrator_vote(&env, &arb1, id.clone(), VoteDecision::DismissOutage, true).unwrap();
        let payout =
            cast_arbitrator_vote(&env, &arb2, id.clone(), VoteDecision::DismissOutage, false)
                .unwrap()
                .expect("quorum should still auto-finalize");

        assert!(!payout.upheld);
        assert_eq!(payout.customer_payout, 1_000);
        assert_eq!(get_arbitration_pool_balance(&env), pool_before);
    }

    #[test]
    fn test_auto_resolve_expired_dispute_favors_filer() {
        let env = Env::default();
        env.mock_all_auths();
        let (_, filer) = setup(&env);

        env.ledger().set_timestamp(1_000);
        let id = Symbol::new(&env, "d1");
        let outage = Symbol::new(&env, "outage");
        open_dispute(&env, &filer, id.clone(), outage, reason(&env), 1_000).unwrap();

        env.ledger()
            .set_timestamp(1_000 + DEFAULT_ARBITRATION_WINDOW_SECS + 1);

        let payout = auto_resolve_expired_dispute(&env, id.clone()).unwrap();
        assert!(payout.upheld);
        assert_eq!(payout.customer_payout, 1_000);

        let details = get_dispute_details(&env, id).unwrap();
        assert_eq!(details.status, DisputeStatus::Resolved);
    }

    #[test]
    fn test_auto_resolve_expired_dispute_rejects_before_deadline() {
        let env = Env::default();
        env.mock_all_auths();
        let (_, filer) = setup(&env);

        env.ledger().set_timestamp(1_000);
        let id = Symbol::new(&env, "d1");
        let outage = Symbol::new(&env, "outage");
        open_dispute(&env, &filer, id.clone(), outage, reason(&env), 1_000).unwrap();

        let result = auto_resolve_expired_dispute(&env, id);
        assert_eq!(result, Err(DisputeError::ArbitrationWindowNotExpired));
    }
}
