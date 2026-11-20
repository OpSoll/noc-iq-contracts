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
    DuplicateResolution,
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
    evaluate_bonus_inner(
        env,
        operator,
        None,
        downtime_seconds,
        window_seconds,
        base_credit,
        budget_remaining,
        multiplier_bps,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn evaluate_bonus_for_resolution(
    env: &Env,
    operator: &Address,
    resolution_id: &Symbol,
    downtime_seconds: u64,
    window_seconds: u64,
    base_credit: i128,
    budget_remaining: i128,
    multiplier_bps: u32,
) -> Result<BonusAward, BonusRejection> {
    evaluate_bonus_inner(
        env,
        operator,
        Some(resolution_id),
        downtime_seconds,
        window_seconds,
        base_credit,
        budget_remaining,
        multiplier_bps,
    )
}

#[allow(clippy::too_many_arguments)]
fn evaluate_bonus_inner(
    env: &Env,
    operator: &Address,
    resolution_id: Option<&Symbol>,
    downtime_seconds: u64,
    window_seconds: u64,
    base_credit: i128,
    budget_remaining: i128,
    multiplier_bps: u32,
) -> Result<BonusAward, BonusRejection> {
    if let Some(id) = resolution_id {
        if crate::outage_correlation_tag::has_bonus_been_awarded(env, id) {
            return Err(BonusRejection::DuplicateResolution);
        }
    }
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

    if let Some(id) = resolution_id {
        if !crate::outage_correlation_tag::mark_bonus_awarded(env, id) {
            return Err(BonusRejection::DuplicateResolution);
        }
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

#[cfg(test)]
mod accrual_tests {
    use super::*;
    use crate::SLACalculatorContract;
    use soroban_sdk::testutils::Address as _;

    const WINDOW: u64 = 2_592_000;

    #[test]
    fn test_bonus_accrues_over_consecutive_clean_windows() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        let operator = Address::generate(&env);
        env.as_contract(&id, || {
            let mut budget = 10_000i128;
            let mut total = 0i128;
            for _ in 0..3 {
                let award = evaluate_bonus(&env, &operator, 0, WINDOW, 1_000, budget, 10_000).unwrap();
                assert!(award.is_payout());
                budget -= award.credit;
                total += award.credit;
            }
            assert_eq!(total, 3_000);
            assert_eq!(budget, 7_000);
        });
    }

    #[test]
    fn test_a_window_with_downtime_earns_nothing_and_does_not_reset_earlier_accrual() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        let operator = Address::generate(&env);
        env.as_contract(&id, || {
            let first = evaluate_bonus(&env, &operator, 0, WINDOW, 1_000, 10_000, 10_000).unwrap();
            let broken = evaluate_bonus(&env, &operator, 30, WINDOW, 1_000, 10_000, 10_000);
            assert_eq!(broken, Err(BonusRejection::IncompleteWindow));
            let third = evaluate_bonus(&env, &operator, 0, WINDOW, 1_000, 10_000, 10_000).unwrap();
            assert_eq!(first.credit + third.credit, 2_000);
        });
    }

    #[test]
    fn test_accrual_is_capped_by_the_remaining_budget() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        let operator = Address::generate(&env);
        env.as_contract(&id, || {
            let a = evaluate_bonus(&env, &operator, 0, WINDOW, 1_000, 1_500, 10_000).unwrap();
            let b = evaluate_bonus(&env, &operator, 0, WINDOW, 1_000, 1_500 - a.credit, 10_000).unwrap();
            assert_eq!(a.credit, 1_000);
            assert_eq!(b.credit, 500);
            let c = evaluate_bonus(&env, &operator, 0, WINDOW, 1_000, 0, 10_000);
            assert_eq!(c, Err(BonusRejection::BonusBudgetDepleted));
        });
    }

    #[test]
    fn test_multiplier_scales_and_clamps_the_accrued_credit() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        let operator = Address::generate(&env);
        env.as_contract(&id, || {
            let half = evaluate_bonus(&env, &operator, 0, WINDOW, 1_000, 10_000, 5_000).unwrap();
            let double = evaluate_bonus(&env, &operator, 0, WINDOW, 1_000, 10_000, 20_000).unwrap();
            assert_eq!((half.credit, double.credit), (500, 2_000));
            assert_eq!(
                evaluate_bonus(&env, &operator, 0, WINDOW, 1_000, 10_000, 20_001),
                Err(BonusRejection::InvalidMultiplier)
            );
        });
    }
}

#[cfg(test)]
mod resolution_dedup_tests {
    use super::*;
    use crate::outage_correlation_tag::has_bonus_been_awarded;
    use crate::SLACalculatorContract;
    use soroban_sdk::{testutils::Address as _, testutils::Events as _, Symbol};

    const WINDOW: u64 = 2_592_000;

    #[test]
    fn one_resolution_pays_at_most_once_and_emits_one_event() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        let operator = Address::generate(&env);
        let resolution = Symbol::new(&env, "RESOLVE1");
        env.as_contract(&id, || {
            let first = evaluate_bonus_for_resolution(
                &env,
                &operator,
                &resolution,
                0,
                WINDOW,
                1_000,
                10_000,
                10_000,
            )
            .unwrap();
            let second = evaluate_bonus_for_resolution(
                &env,
                &operator,
                &resolution,
                0,
                WINDOW,
                1_000,
                10_000 - first.credit,
                10_000,
            );
            assert_eq!(second, Err(BonusRejection::DuplicateResolution));
            assert!(has_bonus_been_awarded(&env, &resolution));
            assert_eq!(env.events().all().len(), 1);
        });
    }

    #[test]
    fn a_distinct_eligible_resolution_is_still_awardable() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        let operator = Address::generate(&env);
        let first = Symbol::new(&env, "RESOLVE1");
        let second = Symbol::new(&env, "RESOLVE2");
        env.as_contract(&id, || {
            let a = evaluate_bonus_for_resolution(
                &env, &operator, &first, 0, WINDOW, 1_000, 10_000, 10_000,
            )
            .unwrap();
            let b = evaluate_bonus_for_resolution(
                &env,
                &operator,
                &second,
                0,
                WINDOW,
                1_000,
                10_000 - a.credit,
                10_000,
            )
            .unwrap();
            assert_eq!((a.credit, b.credit), (1_000, 1_000));
            assert_eq!(env.events().all().len(), 2);
        });
    }

    #[test]
    fn a_denied_resolution_does_not_consume_the_identifier() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        let operator = Address::generate(&env);
        let resolution = Symbol::new(&env, "RESOLVE1");
        env.as_contract(&id, || {
            let denied = evaluate_bonus_for_resolution(
                &env,
                &operator,
                &resolution,
                30,
                WINDOW,
                1_000,
                10_000,
                10_000,
            );
            assert_eq!(denied, Err(BonusRejection::IncompleteWindow));
            assert!(!has_bonus_been_awarded(&env, &resolution));
            let awarded = evaluate_bonus_for_resolution(
                &env,
                &operator,
                &resolution,
                0,
                WINDOW,
                1_000,
                10_000,
                10_000,
            );
            assert_eq!(awarded.unwrap().credit, 1_000);
        });
    }
}
