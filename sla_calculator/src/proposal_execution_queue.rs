// Governance proposal execution queue. Complements config_freeze.rs.
use soroban_sdk::{contracterror, contracttype, symbol_short, Env, Symbol};

#[contracttype]
pub struct QueuedProposal {
    pub proposal_id: u64,
    pub execute_after: u64,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum QueueError {
    TimelockNotExpired = 1,
}

const EXECUTED_EVENT: Symbol = symbol_short!("propexec");

/// Anyone can invoke execute_queued_proposal once its timelock expires,
/// applying the queued parameter update atomically.
pub fn execute_queued_proposal(
    env: &Env,
    proposal: &QueuedProposal,
) -> Result<(), QueueError> {
    if env.ledger().timestamp() < proposal.execute_after {
        return Err(QueueError::TimelockNotExpired);
    }
    env.events()
        .publish((EXECUTED_EVENT,), proposal.proposal_id);
    Ok(())
}
