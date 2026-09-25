// Rate-limited withdrawal safety circuit: max daily withdrawal limit.
// Complements rate_limit.rs.
use soroban_sdk::{contracterror, contracttype, Env};

const TWENTY_FOUR_HOURS_SECS: u64 = 24 * 60 * 60;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum WithdrawalLimitError {
    DailyLimitExceeded = 1,
}

#[contracttype]
pub struct WithdrawalWindow {
    pub window_started_at: u64,
    pub withdrawn_so_far: i128,
}

/// Rejects a withdrawal if it would exceed 10% of vault reserves within
/// the current 24-hour rolling window.
pub fn check_daily_withdrawal_limit(
    env: &Env,
    window: &WithdrawalWindow,
    requested_amount: i128,
    vault_reserves: i128,
) -> Result<(), WithdrawalLimitError> {
    let in_window = env.ledger().timestamp() < window.window_started_at + TWENTY_FOUR_HOURS_SECS;
    let already_withdrawn = if in_window { window.withdrawn_so_far } else { 0 };
    let daily_cap = vault_reserves / 10;
    if already_withdrawn + requested_amount > daily_cap {
        return Err(WithdrawalLimitError::DailyLimitExceeded);
    }
    Ok(())
}
