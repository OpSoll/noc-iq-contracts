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

/// Clamps a per-asset daily limit so it never exceeds the global daily limit.
pub fn clamp_asset_limit(asset_limit: i128, global_limit: i128) -> i128 {
    asset_limit.min(global_limit)
}

#[cfg(test)]
mod override_guard_tests {
    use super::*;

    #[test]
    fn override_above_the_global_limit_is_clamped() {
        assert_eq!(clamp_asset_limit(500, 100), 100);
    }

    #[test]
    fn override_within_the_global_limit_is_kept() {
        assert_eq!(clamp_asset_limit(50, 100), 50);
    }
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

/// Returns the remaining allowance for the current window and when it ends.
pub fn remaining_daily_allowance(
    env: &Env,
    window: &WithdrawalWindow,
    vault_reserves: i128,
) -> (i128, u64) {
    let now = env.ledger().timestamp();
    let window_end = window.window_started_at + TWENTY_FOUR_HOURS_SECS;
    let daily_cap = vault_reserves / 10;
    if now < window_end {
        ((daily_cap - window.withdrawn_so_far).max(0), window_end)
    } else {
        (daily_cap, now + TWENTY_FOUR_HOURS_SECS)
    }
}

#[cfg(test)]
mod window_tests {
    use super::*;

    fn window() -> WithdrawalWindow {
        WithdrawalWindow {
            window_started_at: 1_000,
            withdrawn_so_far: 100,
        }
    }

    #[test]
    fn remaining_allowance_reports_cap_minus_withdrawn_and_window_end() {
        let env = Env::default();
        env.ledger().with_mut(|l| l.timestamp = 1_500);
        let (remaining, window_end) = remaining_daily_allowance(&env, &window(), 2_000);
        assert_eq!(remaining, 100);
        assert_eq!(window_end, 1_000 + TWENTY_FOUR_HOURS_SECS);
    }

    #[test]
    fn allowance_is_still_used_just_before_rollover() {
        let env = Env::default();
        env.ledger()
            .with_mut(|l| l.timestamp = 1_000 + TWENTY_FOUR_HOURS_SECS - 1);
        let result = check_daily_withdrawal_limit(&env, &window(), 1, 1_000);
        assert_eq!(result, Err(WithdrawalLimitError::DailyLimitExceeded));
    }

    #[test]
    fn allowance_resets_just_after_rollover() {
        let env = Env::default();
        env.ledger()
            .with_mut(|l| l.timestamp = 1_000 + TWENTY_FOUR_HOURS_SECS);
        let result = check_daily_withdrawal_limit(&env, &window(), 100, 1_000);
        assert_eq!(result, Ok(()));
    }
}
