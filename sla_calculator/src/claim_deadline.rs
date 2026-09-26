use soroban_sdk::{contracttype, symbol_short, Env, Map, Symbol};

pub const WINDOW_MAP: Symbol = symbol_short!("c_win");
pub const OPENED: Symbol = symbol_short!("opened");
pub const CLAIMED: Symbol = symbol_short!("claimed");
pub const EXPIRED: Symbol = symbol_short!("expired");
pub const BPS_DENOMINATOR: i128 = 10_000;
pub const MAX_CLAIM_BPS: u32 = 20_000;
pub const MAX_WINDOW_S: u64 = 2_592_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
pub enum ClaimState {
    Absent,
    Open,
    Claimed,
    Expired,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct ClaimWindow {
    pub claim_id: Symbol,
    pub opened_at: u64,
    pub deadline: u64,
    pub max_claim_bps: u32,
    pub state: ClaimState,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct Payout {
    pub claim_id: Symbol,
    pub gross: i128,
    pub amount: i128,
    pub claimed_at: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClaimError {
    ZeroWindow,
    WindowTooLong,
    BpsOutOfRange,
    AlreadyOpen,
    NotOpen,
    AlreadyClaimed,
    AlreadyExpired,
    ZeroAmount,
    NegativeAmount,
}

pub fn window_map(env: &Env) -> Map<Symbol, ClaimWindow> {
    env.storage()
        .persistent()
        .get(&WINDOW_MAP)
        .unwrap_or_else(|| Map::new(env))
}

pub fn claim_window(env: &Env, claim_id: &Symbol) -> Option<ClaimWindow> {
    window_map(env).get(claim_id.clone())
}

pub fn claim_state(env: &Env, claim_id: &Symbol, now: u64) -> ClaimState {
    let window = match claim_window(env, claim_id) {
        Some(value) => value,
        None => return ClaimState::Absent,
    };
    match window.state {
        ClaimState::Open => {
            if now > window.deadline {
                ClaimState::Expired
            } else {
                ClaimState::Open
            }
        }
        other => other,
    }
}

pub fn is_claimable(env: &Env, claim_id: &Symbol, now: u64) -> bool {
    claim_state(env, claim_id, now) == ClaimState::Open
}

pub fn seconds_remaining(env: &Env, claim_id: &Symbol, now: u64) -> u32 {
    let window = match claim_window(env, claim_id) {
        Some(value) => value,
        None => return 0,
    };
    if window.deadline <= now {
        return 0;
    }
    let remaining = window.deadline - now;
    if remaining > u64::from(u32::MAX) {
        u32::MAX
    } else {
        remaining as u32
    }
}

pub fn claimable_amount(
    env: &Env,
    claim_id: &Symbol,
    gross: i128,
    now: u64,
) -> Result<i128, ClaimError> {
    if gross < 0 {
        return Err(ClaimError::NegativeAmount);
    }
    if gross == 0 {
        return Err(ClaimError::ZeroAmount);
    }
    if !is_claimable(env, claim_id, now) {
        return Err(match claim_state(env, claim_id, now) {
            ClaimState::Absent | ClaimState::Expired => ClaimError::AlreadyExpired,
            ClaimState::Claimed => ClaimError::AlreadyClaimed,
            ClaimState::Open => ClaimError::NotOpen,
        });
    }
    let window = claim_window(env, claim_id).ok_or(ClaimError::AlreadyExpired)?;
    let capped = (gross * i128::from(window.max_claim_bps)) / BPS_DENOMINATOR;
    if capped < 1 {
        return Ok(1);
    }
    Ok(capped)
}

pub fn open_claim_window(
    env: &Env,
    claim_id: &Symbol,
    opened_at: u64,
    window_seconds: u64,
    max_claim_bps: u32,
) -> Result<u64, ClaimError> {
    if window_seconds == 0 {
        return Err(ClaimError::ZeroWindow);
    }
    if window_seconds > MAX_WINDOW_S {
        return Err(ClaimError::WindowTooLong);
    }
    if max_claim_bps == 0 || max_claim_bps > MAX_CLAIM_BPS {
        return Err(ClaimError::BpsOutOfRange);
    }
    let existing = claim_window(env, claim_id);
    if let Some(window) = &existing {
        if window.state == ClaimState::Open {
            return Err(ClaimError::AlreadyOpen);
        }
        if window.state == ClaimState::Claimed {
            return Err(ClaimError::AlreadyClaimed);
        }
    }
    let deadline = opened_at + window_seconds;
    let window = ClaimWindow {
        claim_id: claim_id.clone(),
        opened_at,
        deadline,
        max_claim_bps,
        state: ClaimState::Open,
    };
    let mut windows = window_map(env);
    windows.set(claim_id.clone(), window);
    env.storage().persistent().set(&WINDOW_MAP, &windows);
    env.events().publish((OPENED, claim_id.clone()), deadline);
    Ok(deadline)
}

pub fn claim(env: &Env, claim_id: &Symbol, gross: i128, now: u64) -> Result<Payout, ClaimError> {
    let amount = claimable_amount(env, claim_id, gross, now)?;
    let mut windows = window_map(env);
    let mut window = windows
        .get(claim_id.clone())
        .ok_or(ClaimError::AlreadyExpired)?;
    window.state = ClaimState::Claimed;
    windows.set(claim_id.clone(), window);
    env.storage().persistent().set(&WINDOW_MAP, &windows);
    let payout = Payout {
        claim_id: claim_id.clone(),
        gross,
        amount,
        claimed_at: now,
    };
    env.events().publish((CLAIMED, claim_id.clone()), amount);
    Ok(payout)
}

pub fn expire_claim(env: &Env, claim_id: &Symbol, now: u64) -> Result<u64, ClaimError> {
    let mut windows = window_map(env);
    let mut window = windows.get(claim_id.clone()).ok_or(ClaimError::NotOpen)?;
    match window.state {
        ClaimState::Claimed => return Err(ClaimError::AlreadyClaimed),
        ClaimState::Expired => return Err(ClaimError::AlreadyExpired),
        _ => {}
    }
    if now <= window.deadline {
        return Err(ClaimError::NotOpen);
    }
    let deadline = window.deadline;
    window.state = ClaimState::Expired;
    windows.set(claim_id.clone(), window);
    env.storage().persistent().set(&WINDOW_MAP, &windows);
    env.events()
        .publish((EXPIRED, claim_id.clone()), now - deadline);
    Ok(now - deadline)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SLACalculatorContract;
    use soroban_sdk::testutils::Events as _;

    fn contract(env: &Env) -> soroban_sdk::Address {
        env.register_contract(None, SLACalculatorContract)
    }

    fn claim_id(env: &Env) -> Symbol {
        Symbol::new(env, "CLM1")
    }

    #[test]
    fn an_unknown_claim_is_absent() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let id = claim_id(&env);
            assert_eq!(claim_state(&env, &id, 1_000), ClaimState::Absent);
            assert!(!is_claimable(&env, &id, 1_000));
            assert_eq!(seconds_remaining(&env, &id, 1_000), 0);
        });
    }

    #[test]
    fn opening_a_window_sets_the_deadline() {
        let env = Env::default();
        let deadline = env.as_contract(&contract(&env), || {
            let id = claim_id(&env);
            open_claim_window(&env, &id, 1_000, 86_400, 10_000)
        });
        assert_eq!(deadline, Ok(87_400));
    }

    #[test]
    fn a_fresh_window_is_claimable() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let id = claim_id(&env);
            open_claim_window(&env, &id, 1_000, 86_400, 10_000).expect("opened");
            assert!(is_claimable(&env, &id, 87_400));
            assert_eq!(seconds_remaining(&env, &id, 1_000), 86_400);
        });
    }

    #[test]
    fn a_zero_window_is_rejected() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let id = claim_id(&env);
            open_claim_window(&env, &id, 1_000, 0, 10_000)
        });
        assert_eq!(result.err(), Some(ClaimError::ZeroWindow));
    }

    #[test]
    fn an_overlong_window_is_rejected() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let id = claim_id(&env);
            open_claim_window(&env, &id, 1_000, MAX_WINDOW_S + 1, 10_000)
        });
        assert_eq!(result.err(), Some(ClaimError::WindowTooLong));
    }

    #[test]
    fn an_out_of_range_ratio_is_rejected() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let id = claim_id(&env);
            open_claim_window(&env, &id, 1_000, 86_400, MAX_CLAIM_BPS + 1)
        });
        assert_eq!(result.err(), Some(ClaimError::BpsOutOfRange));
    }

    #[test]
    fn reopening_an_open_window_is_rejected() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let id = claim_id(&env);
            open_claim_window(&env, &id, 1_000, 86_400, 10_000).expect("opened");
            open_claim_window(&env, &id, 2_000, 86_400, 10_000)
        });
        assert_eq!(result.err(), Some(ClaimError::AlreadyOpen));
    }

    #[test]
    fn a_claim_within_the_deadline_succeeds() {
        let env = Env::default();
        let payout = env.as_contract(&contract(&env), || {
            let id = claim_id(&env);
            open_claim_window(&env, &id, 1_000, 86_400, 10_000).expect("opened");
            claim(&env, &id, 5_000_000, 50_000)
        });
        let entry = payout.expect("payout");
        assert_eq!(entry.amount, 5_000_000);
        assert_eq!(entry.claimed_at, 50_000);
    }

    #[test]
    fn a_claim_after_the_deadline_is_refused() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let id = claim_id(&env);
            open_claim_window(&env, &id, 1_000, 86_400, 10_000).expect("opened");
            claim(&env, &id, 5_000_000, 87_401)
        });
        assert_eq!(result.err(), Some(ClaimError::AlreadyExpired));
    }

    #[test]
    fn the_deadline_instant_is_still_claimable() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let id = claim_id(&env);
            open_claim_window(&env, &id, 1_000, 86_400, 10_000).expect("opened");
            claim(&env, &id, 1_000, 87_400)
        });
        assert!(result.is_ok());
    }

    #[test]
    fn claiming_twice_is_refused() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let id = claim_id(&env);
            open_claim_window(&env, &id, 1_000, 86_400, 10_000).expect("opened");
            claim(&env, &id, 5_000_000, 2_000).expect("claimed");
            claim(&env, &id, 5_000_000, 3_000)
        });
        assert_eq!(result.err(), Some(ClaimError::AlreadyClaimed));
    }

    #[test]
    fn a_partial_ratio_is_honoured() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let id = claim_id(&env);
            open_claim_window(&env, &id, 1_000, 86_400, 5_000).expect("opened");
            let payout = claim(&env, &id, 1_000_000, 2_000).expect("claimed");
            assert_eq!(payout.amount, 500_000);
        });
    }

    #[test]
    fn a_tiny_claim_still_yields_one_unit() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let id = claim_id(&env);
            open_claim_window(&env, &id, 1_000, 86_400, 1).expect("opened");
            let payout = claim(&env, &id, 100, 2_000).expect("claimed");
            assert_eq!(payout.amount, 1);
        });
    }

    #[test]
    fn a_zero_claim_is_refused() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let id = claim_id(&env);
            open_claim_window(&env, &id, 1_000, 86_400, 10_000).expect("opened");
            claim(&env, &id, 0, 2_000)
        });
        assert_eq!(result.err(), Some(ClaimError::ZeroAmount));
    }

    #[test]
    fn a_negative_claim_is_refused() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let id = claim_id(&env);
            open_claim_window(&env, &id, 1_000, 86_400, 10_000).expect("opened");
            claim(&env, &id, -1, 2_000)
        });
        assert_eq!(result.err(), Some(ClaimError::NegativeAmount));
    }

    #[test]
    fn expiring_reports_the_overrun() {
        let env = Env::default();
        let overrun = env.as_contract(&contract(&env), || {
            let id = claim_id(&env);
            open_claim_window(&env, &id, 1_000, 86_400, 10_000).expect("opened");
            expire_claim(&env, &id, 90_000)
        });
        assert_eq!(overrun, Ok(2_600));
    }

    #[test]
    fn expiring_early_is_refused() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let id = claim_id(&env);
            open_claim_window(&env, &id, 1_000, 86_400, 10_000).expect("opened");
            expire_claim(&env, &id, 2_000)
        });
        assert_eq!(result.err(), Some(ClaimError::NotOpen));
    }

    #[test]
    fn expiring_twice_is_refused() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let id = claim_id(&env);
            open_claim_window(&env, &id, 1_000, 86_400, 10_000).expect("opened");
            expire_claim(&env, &id, 90_000).expect("expired");
            expire_claim(&env, &id, 95_000)
        });
        assert_eq!(result.err(), Some(ClaimError::AlreadyExpired));
    }

    #[test]
    fn a_claimed_window_cannot_expire() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let id = claim_id(&env);
            open_claim_window(&env, &id, 1_000, 86_400, 10_000).expect("opened");
            claim(&env, &id, 1_000, 2_000).expect("claimed");
            expire_claim(&env, &id, 90_000)
        });
        assert_eq!(result.err(), Some(ClaimError::AlreadyClaimed));
    }

    #[test]
    fn an_expired_window_can_be_reopened() {
        let env = Env::default();
        let deadline = env.as_contract(&contract(&env), || {
            let id = claim_id(&env);
            open_claim_window(&env, &id, 1_000, 86_400, 10_000).expect("opened");
            expire_claim(&env, &id, 90_000).expect("expired");
            open_claim_window(&env, &id, 90_000, 86_400, 10_000)
        });
        assert_eq!(deadline, Ok(176_400));
    }

    #[test]
    fn the_remaining_time_counts_down_to_zero() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let id = claim_id(&env);
            open_claim_window(&env, &id, 1_000, 10_000, 10_000).expect("opened");
            assert_eq!(seconds_remaining(&env, &id, 5_500), 5_500);
            assert_eq!(seconds_remaining(&env, &id, 11_000), 0);
        });
    }

    #[test]
    fn a_claim_publishes_an_event() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let id = claim_id(&env);
            open_claim_window(&env, &id, 1_000, 86_400, 10_000).expect("opened");
            let before = env.events().all().len();
            claim(&env, &id, 1_000, 2_000).expect("claimed");
            assert_eq!(env.events().all().len(), before + 1);
        });
    }
}
