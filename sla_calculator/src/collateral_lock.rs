use soroban_sdk::{contracttype, symbol_short, Address, Env, Map, Symbol};

pub const COLLATERAL_MAP: Symbol = symbol_short!("coll");
pub const LOCKED: Symbol = symbol_short!("locked");
pub const PAYOUT: Symbol = symbol_short!("payout");
pub const RELEASED: Symbol = symbol_short!("relzd");
pub const BPS_DENOMINATOR: i128 = 10_000;
pub const MAX_COVERAGE_BPS: u32 = 20_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
pub enum LockState {
    Unlocked,
    Locked,
    Released,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct LockRecord {
    pub provider: Address,
    pub amount: i128,
    pub coverage_bps: u32,
    pub state: LockState,
    pub locked_at: u64,
    pub released_at: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LockError {
    ZeroAmount,
    NegativeAmount,
    CoverageOutOfRange,
    AlreadyLocked,
    NotLocked,
    AlreadyReleased,
    ZeroPenalty,
    NegativePenalty,
    PenaltyExceedsCoverage,
    PayoutExceedsLocked,
}

pub fn collateral_map(env: &Env) -> Map<Address, LockRecord> {
    env.storage()
        .persistent()
        .get(&COLLATERAL_MAP)
        .unwrap_or_else(|| Map::new(env))
}

pub fn lock_record(env: &Env, provider: &Address) -> Option<LockRecord> {
    collateral_map(env).get(provider.clone())
}

pub fn lock_state(env: &Env, provider: &Address) -> LockState {
    collateral_map(env)
        .get(provider.clone())
        .map(|record| record.state)
        .unwrap_or(LockState::Unlocked)
}

pub fn available_collateral(env: &Env, provider: &Address) -> i128 {
    match lock_record(env, provider) {
        Some(record) if record.state == LockState::Locked => record.amount,
        _ => 0,
    }
}

pub fn coverage_capacity(env: &Env, provider: &Address) -> i128 {
    let record = match lock_record(env, provider) {
        Some(entry) if entry.state == LockState::Locked => entry,
        _ => return 0,
    };
    (record.amount * i128::from(record.coverage_bps)) / BPS_DENOMINATOR
}

fn store(env: &Env, provider: &Address, record: &LockRecord) {
    let mut records = collateral_map(env);
    records.set(provider.clone(), record.clone());
    env.storage().persistent().set(&COLLATERAL_MAP, &records);
}

pub fn lock_collateral(
    env: &Env,
    provider: &Address,
    amount: i128,
    coverage_bps: u32,
    locked_at: u64,
) -> Result<LockRecord, LockError> {
    if amount < 0 {
        return Err(LockError::NegativeAmount);
    }
    if amount == 0 {
        return Err(LockError::ZeroAmount);
    }
    if coverage_bps == 0 || coverage_bps > MAX_COVERAGE_BPS {
        return Err(LockError::CoverageOutOfRange);
    }
    if lock_state(env, provider) == LockState::Locked {
        return Err(LockError::AlreadyLocked);
    }
    let record = LockRecord {
        provider: provider.clone(),
        amount,
        coverage_bps,
        state: LockState::Locked,
        locked_at,
        released_at: 0,
    };
    store(env, provider, &record);
    env.events().publish((LOCKED, provider.clone()), amount);
    env.events()
        .publish((LOCKED, provider.clone()), coverage_bps);
    Ok(record)
}

pub fn verify_coverage(env: &Env, provider: &Address, penalty: i128) -> Result<i128, LockError> {
    if penalty < 0 {
        return Err(LockError::NegativePenalty);
    }
    if penalty == 0 {
        return Err(LockError::ZeroPenalty);
    }
    if lock_state(env, provider) != LockState::Locked {
        return Err(LockError::NotLocked);
    }
    if penalty > coverage_capacity(env, provider) {
        return Err(LockError::PenaltyExceedsCoverage);
    }
    Ok(penalty)
}

pub fn payout_from_collateral(
    env: &Env,
    provider: &Address,
    penalty: i128,
    paid_at: u64,
) -> Result<i128, LockError> {
    verify_coverage(env, provider, penalty)?;
    let record = lock_record(env, provider).ok_or(LockError::NotLocked)?;
    if penalty > record.amount {
        return Err(LockError::PayoutExceedsLocked);
    }
    let remaining = record.amount - penalty;
    let updated = LockRecord {
        amount: remaining,
        state: if remaining == 0 {
            LockState::Released
        } else {
            LockState::Locked
        },
        released_at: if remaining == 0 { paid_at } else { 0 },
        ..record
    };
    store(env, provider, &updated);
    env.events().publish((PAYOUT, provider.clone()), penalty);
    if remaining == 0 {
        env.events().publish((RELEASED, provider.clone()), 0i128);
    }
    Ok(penalty)
}

pub fn release_collateral(
    env: &Env,
    provider: &Address,
    released_at: u64,
) -> Result<i128, LockError> {
    if lock_state(env, provider) == LockState::Released {
        return Err(LockError::AlreadyReleased);
    }
    let record = lock_record(env, provider).ok_or(LockError::NotLocked)?;
    if record.state != LockState::Locked {
        return Err(LockError::NotLocked);
    }
    let released = record.amount;
    let updated = LockRecord {
        state: LockState::Released,
        released_at,
        ..record
    };
    store(env, provider, &updated);
    env.events().publish((RELEASED, provider.clone()), released);
    Ok(released)
}

pub fn total_locked(env: &Env) -> i128 {
    let mut total = 0i128;
    for (provider, record) in collateral_map(env).iter() {
        if record.state == LockState::Locked {
            let _ = provider;
            total += record.amount;
        }
    }
    total
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
    fn a_provider_starts_unlocked() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let provider = contract(&env);
            assert_eq!(lock_state(&env, &provider), LockState::Unlocked);
            assert_eq!(available_collateral(&env, &provider), 0);
            assert_eq!(coverage_capacity(&env, &provider), 0);
        });
    }

    #[test]
    fn locking_records_the_amount() {
        let env = Env::default();
        let record = env.as_contract(&contract(&env), || {
            let provider = contract(&env);
            lock_collateral(&env, &provider, 1_000_000, 10_000, 100)
        });
        let entry = record.expect("locked");
        assert_eq!(entry.amount, 1_000_000);
        assert_eq!(entry.state, LockState::Locked);
        assert_eq!(entry.locked_at, 100);
    }

    #[test]
    fn a_zero_lock_is_rejected() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let provider = contract(&env);
            lock_collateral(&env, &provider, 0, 10_000, 100)
        });
        assert_eq!(result.err(), Some(LockError::ZeroAmount));
    }

    #[test]
    fn a_negative_lock_is_rejected() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let provider = contract(&env);
            lock_collateral(&env, &provider, -1, 10_000, 100)
        });
        assert_eq!(result.err(), Some(LockError::NegativeAmount));
    }

    #[test]
    fn an_out_of_range_coverage_is_rejected() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let provider = contract(&env);
            lock_collateral(&env, &provider, 1_000, MAX_COVERAGE_BPS + 1, 100)
        });
        assert_eq!(result.err(), Some(LockError::CoverageOutOfRange));
    }

    #[test]
    fn double_locking_is_rejected() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let provider = contract(&env);
            lock_collateral(&env, &provider, 1_000, 10_000, 100).expect("locked");
            lock_collateral(&env, &provider, 2_000, 10_000, 200)
        });
        assert_eq!(result.err(), Some(LockError::AlreadyLocked));
    }

    #[test]
    fn the_coverage_capacity_follows_the_ratio() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let provider = contract(&env);
            lock_collateral(&env, &provider, 1_000_000, 5_000, 100).expect("locked");
            assert_eq!(coverage_capacity(&env, &provider), 500_000);
        });
    }

    #[test]
    fn a_covered_penalty_is_verified() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let provider = contract(&env);
            lock_collateral(&env, &provider, 1_000_000, 10_000, 100).expect("locked");
            assert_eq!(verify_coverage(&env, &provider, 250_000), Ok(250_000));
        });
    }

    #[test]
    fn an_uncovered_penalty_is_refused() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let provider = contract(&env);
            lock_collateral(&env, &provider, 1_000_000, 5_000, 100).expect("locked");
            verify_coverage(&env, &provider, 900_000)
        });
        assert_eq!(result.err(), Some(LockError::PenaltyExceedsCoverage));
    }

    #[test]
    fn a_penalty_without_a_lock_is_refused() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let provider = contract(&env);
            verify_coverage(&env, &provider, 100)
        });
        assert_eq!(result.err(), Some(LockError::NotLocked));
    }

    #[test]
    fn a_zero_penalty_is_refused() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let provider = contract(&env);
            lock_collateral(&env, &provider, 1_000_000, 10_000, 100).expect("locked");
            verify_coverage(&env, &provider, 0)
        });
        assert_eq!(result.err(), Some(LockError::ZeroPenalty));
    }

    #[test]
    fn a_negative_penalty_is_refused() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let provider = contract(&env);
            lock_collateral(&env, &provider, 1_000_000, 10_000, 100).expect("locked");
            verify_coverage(&env, &provider, -5)
        });
        assert_eq!(result.err(), Some(LockError::NegativePenalty));
    }

    #[test]
    fn paying_out_reduces_the_lock() {
        let env = Env::default();
        let paid = env.as_contract(&contract(&env), || {
            let provider = contract(&env);
            lock_collateral(&env, &provider, 1_000_000, 10_000, 100).expect("locked");
            payout_from_collateral(&env, &provider, 250_000, 200)
        });
        assert_eq!(paid, Ok(250_000));
    }

    #[test]
    fn a_fully_drained_lock_is_released() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let provider = contract(&env);
            lock_collateral(&env, &provider, 1_000_000, 10_000, 100).expect("locked");
            payout_from_collateral(&env, &provider, 1_000_000, 200).expect("paid");
            assert_eq!(lock_state(&env, &provider), LockState::Released);
            assert_eq!(available_collateral(&env, &provider), 0);
        });
    }

    #[test]
    fn a_partial_payout_keeps_the_lock() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let provider = contract(&env);
            lock_collateral(&env, &provider, 1_000_000, 10_000, 100).expect("locked");
            payout_from_collateral(&env, &provider, 100_000, 200).expect("paid");
            assert_eq!(lock_state(&env, &provider), LockState::Locked);
            assert_eq!(available_collateral(&env, &provider), 900_000);
        });
    }

    #[test]
    fn releasing_returns_the_remaining_collateral() {
        let env = Env::default();
        let released = env.as_contract(&contract(&env), || {
            let provider = contract(&env);
            lock_collateral(&env, &provider, 750_000, 10_000, 100).expect("locked");
            payout_from_collateral(&env, &provider, 250_000, 200).expect("paid");
            release_collateral(&env, &provider, 300)
        });
        assert_eq!(released, Ok(500_000));
    }

    #[test]
    fn releasing_twice_is_rejected() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let provider = contract(&env);
            lock_collateral(&env, &provider, 750_000, 10_000, 100).expect("locked");
            release_collateral(&env, &provider, 300).expect("released");
            release_collateral(&env, &provider, 400)
        });
        assert_eq!(result.err(), Some(LockError::AlreadyReleased));
    }

    #[test]
    fn releasing_without_a_lock_is_rejected() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let provider = contract(&env);
            release_collateral(&env, &provider, 300)
        });
        assert_eq!(result.err(), Some(LockError::NotLocked));
    }

    #[test]
    fn the_total_locked_sums_every_provider() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let first = contract(&env);
            let second = contract(&env);
            lock_collateral(&env, &first, 400_000, 10_000, 100).expect("locked");
            lock_collateral(&env, &second, 600_000, 10_000, 100).expect("locked");
            assert_eq!(total_locked(&env), 1_000_000);
        });
    }

    #[test]
    fn a_released_lock_drops_out_of_the_total() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let first = contract(&env);
            let second = contract(&env);
            lock_collateral(&env, &first, 400_000, 10_000, 100).expect("locked");
            lock_collateral(&env, &second, 600_000, 10_000, 100).expect("locked");
            release_collateral(&env, &first, 300).expect("released");
            assert_eq!(total_locked(&env), 600_000);
        });
    }

    #[test]
    fn a_payout_publishes_an_event() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let provider = contract(&env);
            lock_collateral(&env, &provider, 1_000_000, 10_000, 100).expect("locked");
            let before = env.events().all().len();
            payout_from_collateral(&env, &provider, 100_000, 200).expect("paid");
            assert_eq!(env.events().all().len(), before + 1);
        });
    }
}
