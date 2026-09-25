// Vault drain protection timelock: 7-day unbonding delay on collateral
// withdrawal requests. Complements reward_cap.rs.
use soroban_sdk::{contracttype, Address, Env};

const SEVEN_DAYS_SECS: u64 = 7 * 24 * 60 * 60;

#[contracttype]
pub struct UnbondingRequest {
    pub operator: Address,
    pub amount: i128,
    pub requested_at: u64,
    pub has_pending_penalty: bool,
}

/// Starts the 7-day unbonding window for a withdrawal request.
pub fn start_unbonding(env: &Env, operator: &Address, amount: i128) -> UnbondingRequest {
    operator.require_auth();
    UnbondingRequest {
        operator: operator.clone(),
        amount,
        requested_at: env.ledger().timestamp(),
        has_pending_penalty: false,
    }
}

/// Withdrawal is executable once 7 days have elapsed and no pending
/// penalty claims lock the collateral.
pub fn is_unbonding_complete(env: &Env, request: &UnbondingRequest) -> bool {
    !request.has_pending_penalty
        && env.ledger().timestamp() >= request.requested_at + SEVEN_DAYS_SECS
}
