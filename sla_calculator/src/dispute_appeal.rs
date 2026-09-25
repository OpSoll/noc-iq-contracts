// Dispute outcome appeal mechanism to secondary council. Complements
// dispute.rs.
use soroban_sdk::{contracterror, symbol_short, Address, Env, Symbol};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum AppealError {
    WindowExpired = 1,
    AlreadyAppealed = 2,
}

const FORTY_EIGHT_HOURS_SECS: u64 = 48 * 60 * 60;
const APPEAL_FILED_EVENT: Symbol = symbol_short!("appeal_f");

/// Losing party files a one-time appeal within 48 hours of settlement by
/// depositing 2x the original appeal bond, escalating to the Senior
/// Governance Council for a final binding decision.
pub fn file_appeal(
    env: &Env,
    appellant: &Address,
    dispute_id: u64,
    settled_at: u64,
    already_appealed: bool,
    original_bond: i128,
) -> Result<i128, AppealError> {
    appellant.require_auth();
    if already_appealed {
        return Err(AppealError::AlreadyAppealed);
    }
    if env.ledger().timestamp() > settled_at + FORTY_EIGHT_HOURS_SECS {
        return Err(AppealError::WindowExpired);
    }
    let required_bond = original_bond * 2;
    env.events().publish((APPEAL_FILED_EVENT,), dispute_id);
    Ok(required_bond)
}
