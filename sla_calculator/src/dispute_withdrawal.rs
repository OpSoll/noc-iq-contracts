// Dispute withdrawal mechanism before voting begins. Complements
// dispute.rs.
use soroban_sdk::{contracterror, symbol_short, Address, Env, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum WithdrawalError {
    WindowExpired = 1,
    VotesAlreadyCast = 2,
}

const TWO_HOURS_SECS: u64 = 2 * 60 * 60;
const WITHDRAWN_EVENT: Symbol = symbol_short!("dwithdrw");

/// Submitter withdraws a dispute within 2 hours, refunding 95% of the
/// bond, provided no arbitrator votes have been recorded yet.
pub fn withdraw_dispute(
    env: &Env,
    submitter: &Address,
    dispute_id: u64,
    filed_at: u64,
    votes_cast: u32,
    bond_amount: i128,
) -> Result<i128, WithdrawalError> {
    submitter.require_auth();
    if env.ledger().timestamp() > filed_at + TWO_HOURS_SECS {
        return Err(WithdrawalError::WindowExpired);
    }
    if votes_cast > 0 {
        return Err(WithdrawalError::VotesAlreadyCast);
    }
    let refund = bond_amount * 95 / 100;
    env.events().publish((WITHDRAWN_EVENT,), dispute_id);
    Ok(refund)
}
