use soroban_sdk::{symbol_short, Address, Env, Symbol};

pub const BONUS_EARNED: Symbol = symbol_short!("bonus");
pub const BONUS_DENIED: Symbol = symbol_short!("b_deny");
pub const MAX_MULTIPLIER_BPS: u32 = 20_000;
pub const BPS_DENOMINATOR: u32 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BonusRejection {
    BonusBudgetDepleted,
    IncompleteWindow,
    InvalidMultiplier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BonusAward {
    pub operator: Address,
    pub credit: i128,
    pub multiplier_bps: u32,
    pub window_seconds: u64,
}

impl BonusAward {
    pub fn is_payout(&self) -> bool {
        self.credit > 0
    }
}

pub fn is_eligible(downtime_seconds: u64, window_seconds: u64) -> bool {
    downtime_seconds == 0 && window_seconds > 0
}

pub fn clamp_multiplier(multiplier_bps: u32) -> u32 {
    multiplier_bps.min(MAX_MULTIPLIER_BPS)
}

pub fn scaled_credit(base_credit: i128, multiplier_bps: u32) -> i128 {
    let scaled = base_credit
        .saturating_mul(clamp_multiplier(multiplier_bps) as i128)
        .checked_div(BPS_DENOMINATOR as i128)
        .unwrap_or(0);
    scaled.max(0)
}

pub fn cap_to_budget(credit: i128, budget_remaining: i128) -> i128 {
    credit.min(budget_remaining).max(0)
}

pub fn evaluate_bonus(
    env: &Env,
    operator: &Address,
    downtime_seconds: u64,
    window_seconds: u64,
    base_credit: i128,
    budget_remaining: i128,
    multiplier_bps: u32,
) -> Result<BonusAward, BonusRejection> {
    if multiplier_bps == 0 || multiplier_bps > MAX_MULTIPLIER_BPS {
        env.events()
            .publish((BONUS_DENIED, operator.clone()), multiplier_bps);
        return Err(BonusRejection::InvalidMultiplier);
    }
    if budget_remaining <= 0 {
        env.events()
            .publish((BONUS_DENIED, operator.clone()), budget_remaining);
        return Err(BonusRejection::BonusBudgetDepleted);
    }
    if !is_eligible(downtime_seconds, window_seconds) {
        env.events()
            .publish((BONUS_DENIED, operator.clone()), downtime_seconds);
        return Err(BonusRejection::IncompleteWindow);
    }

    let credit = cap_to_budget(scaled_credit(base_credit, multiplier_bps), budget_remaining);
    let award = BonusAward {
        operator: operator.clone(),
        credit,
        multiplier_bps: clamp_multiplier(multiplier_bps),
        window_seconds,
    };
    env.events()
        .publish((BONUS_EARNED, operator.clone()), award.credit);
    Ok(award)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SLACalculatorContract;
    use soroban_sdk::{
        testutils::{Address as _, Events as _},
        Address,
    };

    const WINDOW: u64 = 2_592_000;

    fn setup(env: &Env) -> (Address, Address) {
        let contract = env.register_contract(None, SLACalculatorContract);
        let operator = Address::generate(env);
        (contract, operator)
    }

    #[test]
    fn eligibility_requires_zero_downtime_and_a_full_window() {
        assert!(is_eligible(0, WINDOW));
        assert!(!is_eligible(1, WINDOW));
        assert!(!is_eligible(0, 0));
    }

    #[test]
    fn multiplier_is_capped() {
        assert_eq!(clamp_multiplier(15_000), 15_000);
        assert_eq!(clamp_multiplier(50_000), MAX_MULTIPLIER_BPS);
    }

    #[test]
    fn credit_scales_with_the_multiplier() {
        assert_eq!(scaled_credit(100, 15_000), 150);
        assert_eq!(scaled_credit(100, 10_000), 100);
        assert_eq!(scaled_credit(100, 0), 0);
        assert_eq!(scaled_credit(-100, 10_000), 0);
    }

    #[test]
    fn credit_never_exceeds_the_remaining_budget() {
        assert_eq!(cap_to_budget(150, 120), 120);
        assert_eq!(cap_to_budget(150, 500), 150);
        assert_eq!(cap_to_budget(150, 0), 0);
    }

    #[test]
    fn perfect_uptime_earns_the_bonus() {
        let env = Env::default();
        let (contract, operator) = setup(&env);
        let award = env.as_contract(&contract, || {
            evaluate_bonus(&env, &operator, 0, WINDOW, 100, 1_000, 15_000)
        });
        let award = award.expect("eligible window");
        assert_eq!(award.credit, 150);
        assert_eq!(award.operator, operator);
        assert!(award.is_payout());
    }

    #[test]
    fn any_downtime_blocks_the_bonus() {
        let env = Env::default();
        let (contract, operator) = setup(&env);
        let result = env.as_contract(&contract, || {
            evaluate_bonus(&env, &operator, 30, WINDOW, 100, 1_000, 10_000)
        });
        assert_eq!(result, Err(BonusRejection::IncompleteWindow));
    }

    #[test]
    fn a_depleted_budget_disables_the_bonus() {
        let env = Env::default();
        let (contract, operator) = setup(&env);
        let result = env.as_contract(&contract, || {
            evaluate_bonus(&env, &operator, 0, WINDOW, 100, 0, 10_000)
        });
        assert_eq!(result, Err(BonusRejection::BonusBudgetDepleted));
    }

    #[test]
    fn an_out_of_range_multiplier_is_rejected() {
        let env = Env::default();
        let (contract, operator) = setup(&env);
        let result = env.as_contract(&contract, || {
            evaluate_bonus(&env, &operator, 0, WINDOW, 100, 1_000, 0)
        });
        assert_eq!(result, Err(BonusRejection::InvalidMultiplier));
    }

    #[test]
    fn the_award_is_capped_by_the_budget() {
        let env = Env::default();
        let (contract, operator) = setup(&env);
        let award = env
            .as_contract(&contract, || {
                evaluate_bonus(&env, &operator, 0, WINDOW, 100, 120, 15_000)
            })
            .expect("eligible window");
        assert_eq!(award.credit, 120);
    }

    #[test]
    fn a_paid_bonus_emits_bonus_earned() {
        let env = Env::default();
        let (contract, operator) = setup(&env);
        env.as_contract(&contract, || {
            evaluate_bonus(&env, &operator, 0, WINDOW, 100, 1_000, 15_000)
        })
        .expect("eligible window");
        assert_eq!(env.events().all().len(), 1);
    }

    #[test]
    fn a_denied_bonus_still_emits_a_denial_event() {
        let env = Env::default();
        let (contract, operator) = setup(&env);
        let denied = env.as_contract(&contract, || {
            evaluate_bonus(&env, &operator, 5, WINDOW, 100, 1_000, 10_000)
        });
        assert!(denied.is_err());
        assert_eq!(env.events().all().len(), 1);
    }
}
