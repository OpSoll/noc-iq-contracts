use soroban_sdk::{contracttype, symbol_short, Address, Env, Map, Symbol};

pub const BREACH_MAP: Symbol = symbol_short!("b_rec");
pub const STRIKE: Symbol = symbol_short!("strike");
pub const FACTOR: Symbol = symbol_short!("f_mult");
pub const CLEARED: Symbol = symbol_short!("cleared");
pub const BPS_DENOMINATOR: u32 = 10_000;
pub const FIRST_BREACH_FACTOR_BPS: u32 = 10_000;
pub const STEP_FACTOR_BPS: u32 = 2_000;
pub const MAX_FACTOR_BPS: u32 = 28_000;
pub const MAX_STRIKES: u32 = 10;
pub const MAX_PENALTY_BPS: u32 = 20_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EscalationError {
    ZeroPenalty,
    PenaltyOutOfRange,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct BreachRecord {
    pub breaches: u32,
    pub factor_bps: u32,
    pub last_breach_at: u64,
    pub escalated_penalty_bps: u32,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct PenaltyTerms {
    pub base_penalty_bps: u32,
    pub factor_bps: u32,
    pub effective_penalty_bps: u32,
}

pub fn breach_map(env: &Env) -> Map<Address, BreachRecord> {
    env.storage()
        .persistent()
        .get(&BREACH_MAP)
        .unwrap_or_else(|| Map::new(env))
}

pub fn breach_record(env: &Env, provider: &Address) -> Option<BreachRecord> {
    breach_map(env).get(provider.clone())
}

pub fn breach_count(env: &Env, provider: &Address) -> u32 {
    breach_map(env)
        .get(provider.clone())
        .map(|record| record.breaches)
        .unwrap_or(0)
}

pub fn escalation_factor_bps(breaches: u32) -> u32 {
    if breaches == 0 {
        return FIRST_BREACH_FACTOR_BPS;
    }
    let capped = if breaches > MAX_STRIKES {
        MAX_STRIKES
    } else {
        breaches
    };
    let factor = FIRST_BREACH_FACTOR_BPS + (capped - 1) * STEP_FACTOR_BPS;
    if factor > MAX_FACTOR_BPS {
        MAX_FACTOR_BPS
    } else {
        factor
    }
}

pub fn effective_penalty_bps(base_penalty_bps: u32, factor_bps: u32) -> u32 {
    let scaled = (u64::from(base_penalty_bps) * u64::from(factor_bps)) / u64::from(BPS_DENOMINATOR);
    if scaled > u64::from(MAX_PENALTY_BPS) {
        MAX_PENALTY_BPS
    } else {
        scaled as u32
    }
}

pub fn penalty_terms(env: &Env, provider: &Address, base_penalty_bps: u32) -> PenaltyTerms {
    let factor_bps = escalation_factor_bps(breach_count(env, provider));
    PenaltyTerms {
        base_penalty_bps,
        factor_bps,
        effective_penalty_bps: effective_penalty_bps(base_penalty_bps, factor_bps),
    }
}

pub fn record_breach(
    env: &Env,
    provider: &Address,
    base_penalty_bps: u32,
    breached_at: u64,
) -> Result<BreachRecord, EscalationError> {
    if base_penalty_bps == 0 {
        return Err(EscalationError::ZeroPenalty);
    }
    if base_penalty_bps > MAX_PENALTY_BPS {
        return Err(EscalationError::PenaltyOutOfRange);
    }
    let previous = breach_record(env, provider);
    let breaches = match &previous {
        Some(record) if record.breaches < MAX_STRIKES => record.breaches + 1,
        Some(_) => MAX_STRIKES,
        None => 1,
    };
    let factor_bps = escalation_factor_bps(breaches);
    let updated = BreachRecord {
        breaches,
        factor_bps,
        last_breach_at: breached_at,
        escalated_penalty_bps: effective_penalty_bps(base_penalty_bps, factor_bps),
    };
    let mut records = breach_map(env);
    records.set(provider.clone(), updated.clone());
    env.storage().persistent().set(&BREACH_MAP, &records);
    env.events().publish((STRIKE, provider.clone()), breaches);
    env.events().publish((FACTOR, provider.clone()), factor_bps);
    Ok(updated)
}

pub fn clear_strikes(env: &Env, provider: &Address) -> u32 {
    let cleared = breach_count(env, provider);
    let mut records = breach_map(env);
    records.remove(provider.clone());
    env.storage().persistent().set(&BREACH_MAP, &records);
    env.events().publish((CLEARED, provider.clone()), cleared);
    cleared
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SLACalculatorContract;
    use soroban_sdk::testutils::Events as _;

    fn contract(env: &Env) -> soroban_sdk::Address {
        env.register_contract(None, SLACalculatorContract)
    }

    #[test]
    fn a_first_breach_is_not_escalated() {
        assert_eq!(escalation_factor_bps(1), FIRST_BREACH_FACTOR_BPS);
    }

    #[test]
    fn no_breach_means_the_base_factor() {
        assert_eq!(escalation_factor_bps(0), FIRST_BREACH_FACTOR_BPS);
    }

    #[test]
    fn each_breach_adds_a_step() {
        assert_eq!(escalation_factor_bps(2), 12_000);
        assert_eq!(escalation_factor_bps(3), 14_000);
        assert_eq!(escalation_factor_bps(4), 16_000);
    }

    #[test]
    fn the_factor_is_capped() {
        assert_eq!(escalation_factor_bps(MAX_STRIKES), MAX_FACTOR_BPS);
        assert_eq!(escalation_factor_bps(50), MAX_FACTOR_BPS);
    }

    #[test]
    fn the_penalty_is_scaled_by_the_factor() {
        assert_eq!(effective_penalty_bps(1_000, 10_000), 1_000);
        assert_eq!(effective_penalty_bps(1_000, 12_000), 1_200);
        assert_eq!(effective_penalty_bps(2_000, 12_500), 2_500);
    }

    #[test]
    fn the_effective_penalty_is_capped() {
        assert_eq!(effective_penalty_bps(20_000, 30_000), MAX_PENALTY_BPS);
    }

    #[test]
    fn scaling_never_truncates_to_zero() {
        assert!(effective_penalty_bps(1, FIRST_BREACH_FACTOR_BPS) >= 1);
    }

    #[test]
    fn a_fresh_provider_has_no_history() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let provider = contract(&env);
            assert_eq!(breach_count(&env, &provider), 0);
            assert!(breach_record(&env, &provider).is_none());
            let terms = penalty_terms(&env, &provider, 1_000);
            assert_eq!(terms.effective_penalty_bps, 1_000);
        });
    }

    #[test]
    fn recording_a_breach_starts_the_streak() {
        let env = Env::default();
        let record = env.as_contract(&contract(&env), || {
            let provider = contract(&env);
            record_breach(&env, &provider, 1_000, 500)
        });
        assert_eq!(record.map(|entry| entry.breaches), Ok(1));
    }

    #[test]
    fn repeated_breaches_escalate() {
        let env = Env::default();
        let records = env.as_contract(&contract(&env), || {
            let provider = contract(&env);
            let mut last = None;
            for index in 0..3u32 {
                last = Some(
                    record_breach(&env, &provider, 1_000, 100 * u64::from(index)).expect("breach"),
                );
            }
            last
        });
        let record = records.expect("record");
        assert_eq!(record.breaches, 3);
        assert_eq!(record.factor_bps, 14_000);
        assert_eq!(record.escalated_penalty_bps, 1_400);
    }

    #[test]
    fn the_streak_never_runs_past_the_cap() {
        let env = Env::default();
        let breaches = env.as_contract(&contract(&env), || {
            let provider = contract(&env);
            for index in 0..(MAX_STRIKES + 3) {
                record_breach(&env, &provider, 1_000, u64::from(index)).expect("breach");
            }
            breach_count(&env, &provider)
        });
        assert_eq!(breaches, MAX_STRIKES);
    }

    #[test]
    fn a_zero_penalty_is_rejected() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let provider = contract(&env);
            record_breach(&env, &provider, 0, 10)
        });
        assert_eq!(result.err(), Some(EscalationError::ZeroPenalty));
    }

    #[test]
    fn an_oversized_base_penalty_is_rejected() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let provider = contract(&env);
            record_breach(&env, &provider, MAX_PENALTY_BPS + 1, 10)
        });
        assert_eq!(result.err(), Some(EscalationError::PenaltyOutOfRange));
    }

    #[test]
    fn clearing_resets_the_streak() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let provider = contract(&env);
            record_breach(&env, &provider, 1_000, 10).expect("breach");
            record_breach(&env, &provider, 1_000, 20).expect("breach");
            assert_eq!(clear_strikes(&env, &provider), 2);
            assert_eq!(breach_count(&env, &provider), 0);
            let terms = penalty_terms(&env, &provider, 1_000);
            assert_eq!(terms.effective_penalty_bps, 1_000);
        });
    }

    #[test]
    fn providers_escalate_independently() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let first = contract(&env);
            let second = contract(&env);
            record_breach(&env, &first, 1_000, 10).expect("breach");
            record_breach(&env, &first, 1_000, 20).expect("breach");
            record_breach(&env, &second, 1_000, 30).expect("breach");
            assert_eq!(breach_count(&env, &first), 2);
            assert_eq!(breach_count(&env, &second), 1);
        });
    }

    #[test]
    fn a_breach_publishes_two_events() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let provider = contract(&env);
            record_breach(&env, &provider, 1_000, 10).expect("breach");
            assert_eq!(env.events().all().len(), 2);
        });
    }
}
