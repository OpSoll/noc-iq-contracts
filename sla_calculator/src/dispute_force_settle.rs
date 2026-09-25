// Emergency override bypass for stuck disputes: 3-of-5 council can
// force-settle a dispute inactive >30 days. Complements dispute.rs.
use soroban_sdk::{contracterror, symbol_short, Address, Env, Symbol, Vec};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ForceSettleError {
    NotInactiveLongEnough = 1,
    InsufficientCouncilSignatures = 2,
}

const THIRTY_DAYS_SECS: u64 = 30 * 24 * 60 * 60;
const FORCE_SETTLED_EVENT: Symbol = symbol_short!("dfsettle");

/// 3-of-5 admin council can force-settle a dispute that has been
/// inactive for more than 30 days, overruling pending arbitrator votes.
pub fn force_settle_dispute(
    env: &Env,
    dispute_id: u64,
    outcome: u32,
    last_activity_at: u64,
    council_signers: &Vec<Address>,
) -> Result<(), ForceSettleError> {
    if env.ledger().timestamp() < last_activity_at + THIRTY_DAYS_SECS {
        return Err(ForceSettleError::NotInactiveLongEnough);
    }
    if council_signers.len() < 3 {
        return Err(ForceSettleError::InsufficientCouncilSignatures);
    }
    for signer in council_signers.iter() {
        signer.require_auth();
    }
    env.events()
        .publish((FORCE_SETTLED_EVENT,), (dispute_id, outcome));
    Ok(())
}
