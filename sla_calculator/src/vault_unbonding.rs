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

/// Amount still locked by an unbonding request: the full amount until the
/// request is executable, and `0` once it can be withdrawn.
pub fn pending_unbonding_amount(env: &Env, request: &UnbondingRequest) -> i128 {
    if is_unbonding_complete(env, request) {
        0
    } else {
        request.amount
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SLACalculatorContract;
    use soroban_sdk::testutils::{Address as _, Ledger as _};

    fn setup() -> (Env, Address, Address) {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().with_mut(|l| l.timestamp = 1_000);
        let id = env.register_contract(None, SLACalculatorContract);
        let operator = Address::generate(&env);
        (env, id, operator)
    }

    #[test]
    fn test_not_complete_one_second_before_seven_days() {
        let (env, id, op) = setup();
        let req = env.as_contract(&id, || start_unbonding(&env, &op, 500));
        env.ledger().with_mut(|l| l.timestamp = 1_000 + SEVEN_DAYS_SECS - 1);
        assert!(!is_unbonding_complete(&env, &req));
    }

    #[test]
    fn test_complete_exactly_at_seven_days() {
        let (env, id, op) = setup();
        let req = env.as_contract(&id, || start_unbonding(&env, &op, 500));
        env.ledger().with_mut(|l| l.timestamp = 1_000 + SEVEN_DAYS_SECS);
        assert!(is_unbonding_complete(&env, &req));
    }

    #[test]
    fn test_pending_penalty_blocks_completion_after_the_delay() {
        let (env, id, op) = setup();
        let mut req = env.as_contract(&id, || start_unbonding(&env, &op, 500));
        req.has_pending_penalty = true;
        env.ledger().with_mut(|l| l.timestamp = 1_000 + SEVEN_DAYS_SECS * 2);
        assert!(!is_unbonding_complete(&env, &req));
    }

    #[test]
    fn test_pending_amount_view_drops_to_zero_once_executable() {
        let (env, id, op) = setup();
        let req = env.as_contract(&id, || start_unbonding(&env, &op, 500));
        assert_eq!(pending_unbonding_amount(&env, &req), 500);
        env.ledger().with_mut(|l| l.timestamp = 1_000 + SEVEN_DAYS_SECS);
        assert_eq!(pending_unbonding_amount(&env, &req), 0);
    }
}
