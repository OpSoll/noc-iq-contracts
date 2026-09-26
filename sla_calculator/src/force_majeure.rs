use soroban_sdk::{contracttype, symbol_short, Address, Env, Map, Symbol};

pub const GOVERNANCE_KEY: Symbol = symbol_short!("ADMIN");
pub const FORCED: Symbol = symbol_short!("forced");
pub const EXCUSE: Symbol = symbol_short!("excuse");
pub const CLOSED: Symbol = symbol_short!("closed");
pub const MAX_EXCUSE_S: u64 = 86_400;
pub const MAX_RESIDUAL_BPS: u32 = 10_000;
pub const MIN_REASON_LEN: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
pub enum EventKind {
    Outage,
    Degraded,
    Maintenance,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForceMajeureError {
    NotGovernance,
    NoGovernance,
    WindowTooLong,
    WindowTooShort,
    EventTooLarge,
    AlreadyRecorded,
    NotRecorded,
    AlreadyClosed,
    StillRunning,
    ReasonTooShort,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct ForceMajeureRecord {
    pub outage_id: Symbol,
    pub kind: EventKind,
    pub started_at: u64,
    pub excused_until: u64,
    pub reason: Symbol,
    pub approved_by: Address,
    pub residual_penalty_bps: u32,
    pub closed_at: u64,
}

pub fn record_map(env: &Env) -> Map<Symbol, ForceMajeureRecord> {
    env.storage()
        .persistent()
        .get(&symbol_short!("fm_rec"))
        .unwrap_or_else(|| Map::new(env))
}

pub fn set_governance(env: &Env, admin: &Address) {
    env.storage().persistent().set(&GOVERNANCE_KEY, admin);
}

pub fn governance(env: &Env) -> Option<Address> {
    env.storage().persistent().get(&GOVERNANCE_KEY)
}

pub fn is_governance(env: &Env, caller: &Address) -> bool {
    governance(env)
        .map(|admin| admin == *caller)
        .unwrap_or(false)
}

pub fn record(env: &Env, outage_id: &Symbol) -> Option<ForceMajeureRecord> {
    record_map(env).get(outage_id.clone())
}

pub fn is_excused(env: &Env, outage_id: &Symbol, at: u64) -> bool {
    match record(env, outage_id) {
        Some(entry) => at >= entry.started_at && at <= entry.excused_until,
        None => false,
    }
}

pub fn is_open(env: &Env, outage_id: &Symbol) -> bool {
    match record(env, outage_id) {
        Some(entry) => entry.closed_at == 0,
        None => false,
    }
}

pub fn apply_penalty_bps(env: &Env, outage_id: &Symbol, base_penalty_bps: u32, at: u64) -> u32 {
    if is_excused(env, outage_id, at) {
        return 0;
    }
    base_penalty_bps
}

#[allow(clippy::too_many_arguments)]
pub fn record_force_majeure(
    env: &Env,
    caller: &Address,
    outage_id: &Symbol,
    kind: EventKind,
    started_at: u64,
    excused_until: u64,
    reason: Symbol,
    residual_penalty_bps: u32,
) -> Result<ForceMajeureRecord, ForceMajeureError> {
    if !is_governance(env, caller) {
        return Err(ForceMajeureError::NotGovernance);
    }
    if is_open(env, outage_id) {
        return Err(ForceMajeureError::AlreadyRecorded);
    }
    if excused_until <= started_at {
        return Err(ForceMajeureError::WindowTooShort);
    }
    if excused_until - started_at > MAX_EXCUSE_S {
        return Err(ForceMajeureError::WindowTooLong);
    }
    if residual_penalty_bps > MAX_RESIDUAL_BPS {
        return Err(ForceMajeureError::EventTooLarge);
    }
    if kind != EventKind::Outage && residual_penalty_bps > 0 && reason == Symbol::new(env, "") {
        return Err(ForceMajeureError::ReasonTooShort);
    }
    let entry = ForceMajeureRecord {
        outage_id: outage_id.clone(),
        kind,
        started_at,
        excused_until,
        reason,
        approved_by: caller.clone(),
        residual_penalty_bps,
        closed_at: 0,
    };
    let mut records = record_map(env);
    records.set(outage_id.clone(), entry.clone());
    env.storage()
        .persistent()
        .set(&symbol_short!("fm_rec"), &records);
    env.events()
        .publish((FORCED, outage_id.clone()), excused_until);
    Ok(entry)
}

pub fn close_force_majeure(
    env: &Env,
    caller: &Address,
    outage_id: &Symbol,
    at: u64,
) -> Result<u64, ForceMajeureError> {
    if !is_governance(env, caller) {
        return Err(ForceMajeureError::NotGovernance);
    }
    let entry = record(env, outage_id).ok_or(ForceMajeureError::NotRecorded)?;
    if entry.closed_at != 0 {
        return Err(ForceMajeureError::AlreadyClosed);
    }
    if at < entry.excused_until {
        return Err(ForceMajeureError::StillRunning);
    }
    let closure = at.min(entry.excused_until);
    let updated = ForceMajeureRecord {
        closed_at: at,
        ..entry
    };
    let mut records = record_map(env);
    records.set(outage_id.clone(), updated);
    env.storage()
        .persistent()
        .set(&symbol_short!("fm_rec"), &records);
    env.events().publish((EXCUSE, outage_id.clone()), closure);
    env.events().publish((CLOSED, outage_id.clone()), at);
    Ok(closure)
}

pub fn excused_seconds(env: &Env, outage_id: &Symbol, at: u64) -> u64 {
    let entry = match record(env, outage_id) {
        Some(value) => value,
        None => return 0,
    };
    if at <= entry.started_at {
        return 0;
    }
    let end = if entry.closed_at != 0 && entry.closed_at < entry.excused_until {
        entry.closed_at
    } else {
        entry.excused_until
    };
    if at <= end {
        at - entry.started_at
    } else {
        end.saturating_sub(entry.started_at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SLACalculatorContract;

    fn contract(env: &Env) -> soroban_sdk::Address {
        env.register_contract(None, SLACalculatorContract)
    }

    fn reason(env: &Env) -> Symbol {
        Symbol::new(env, "quake")
    }

    #[test]
    fn a_full_outage_is_penalty_free_inside_the_window() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let admin = contract(&env);
            let outage = Symbol::new(&env, "OUT1");
            set_governance(&env, &admin);
            record_force_majeure(
                &env,
                &admin,
                &outage,
                EventKind::Outage,
                1_000,
                5_000,
                reason(&env),
                0,
            )
            .expect("recorded");
            assert!(is_excused(&env, &outage, 3_000));
            assert_eq!(apply_penalty_bps(&env, &outage, 5_000, 3_000), 0);
        });
    }

    #[test]
    fn the_penalty_applies_again_after_the_window() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let admin = contract(&env);
            let outage = Symbol::new(&env, "OUT1");
            set_governance(&env, &admin);
            record_force_majeure(
                &env,
                &admin,
                &outage,
                EventKind::Outage,
                1_000,
                5_000,
                reason(&env),
                0,
            )
            .expect("recorded");
            assert!(!is_excused(&env, &outage, 5_001));
            assert_eq!(apply_penalty_bps(&env, &outage, 5_000, 5_001), 5_000);
        });
    }

    #[test]
    fn the_penalty_applies_before_the_event_starts() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let admin = contract(&env);
            let outage = Symbol::new(&env, "OUT1");
            set_governance(&env, &admin);
            record_force_majeure(
                &env,
                &admin,
                &outage,
                EventKind::Outage,
                1_000,
                5_000,
                reason(&env),
                0,
            )
            .expect("recorded");
            assert_eq!(apply_penalty_bps(&env, &outage, 5_000, 999), 5_000);
        });
    }

    #[test]
    fn a_degraded_event_keeps_its_residual_penalty() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let admin = contract(&env);
            let outage = Symbol::new(&env, "OUT1");
            set_governance(&env, &admin);
            let entry = record_force_majeure(
                &env,
                &admin,
                &outage,
                EventKind::Degraded,
                1_000,
                2_000,
                reason(&env),
                500,
            )
            .expect("recorded");
            assert_eq!(entry.residual_penalty_bps, 500);
        });
    }

    #[test]
    fn a_stranger_cannot_approve_an_excuse() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let admin = contract(&env);
            let stranger = contract(&env);
            set_governance(&env, &admin);
            let outage = Symbol::new(&env, "OUT1");
            record_force_majeure(
                &env,
                &stranger,
                &outage,
                EventKind::Outage,
                1_000,
                2_000,
                reason(&env),
                0,
            )
        });
        assert_eq!(result.err(), Some(ForceMajeureError::NotGovernance));
    }

    #[test]
    fn an_unset_governance_blocks_everyone() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let admin = contract(&env);
            let outage = Symbol::new(&env, "OUT1");
            record_force_majeure(
                &env,
                &admin,
                &outage,
                EventKind::Outage,
                1_000,
                2_000,
                reason(&env),
                0,
            )
        });
        assert_eq!(result.err(), Some(ForceMajeureError::NotGovernance));
    }

    #[test]
    fn an_overlong_window_is_rejected() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let admin = contract(&env);
            set_governance(&env, &admin);
            let outage = Symbol::new(&env, "OUT1");
            record_force_majeure(
                &env,
                &admin,
                &outage,
                EventKind::Outage,
                1_000,
                1_000 + MAX_EXCUSE_S + 1,
                reason(&env),
                0,
            )
        });
        assert_eq!(result.err(), Some(ForceMajeureError::WindowTooLong));
    }

    #[test]
    fn an_empty_window_is_rejected() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let admin = contract(&env);
            set_governance(&env, &admin);
            let outage = Symbol::new(&env, "OUT1");
            record_force_majeure(
                &env,
                &admin,
                &outage,
                EventKind::Outage,
                1_000,
                1_000,
                reason(&env),
                0,
            )
        });
        assert_eq!(result.err(), Some(ForceMajeureError::WindowTooShort));
    }

    #[test]
    fn an_oversized_residual_penalty_is_rejected() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let admin = contract(&env);
            set_governance(&env, &admin);
            let outage = Symbol::new(&env, "OUT1");
            record_force_majeure(
                &env,
                &admin,
                &outage,
                EventKind::Degraded,
                1_000,
                2_000,
                reason(&env),
                MAX_RESIDUAL_BPS + 1,
            )
        });
        assert_eq!(result.err(), Some(ForceMajeureError::EventTooLarge));
    }

    #[test]
    fn a_duplicate_record_is_rejected() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let admin = contract(&env);
            set_governance(&env, &admin);
            let outage = Symbol::new(&env, "OUT1");
            record_force_majeure(
                &env,
                &admin,
                &outage,
                EventKind::Outage,
                1_000,
                2_000,
                reason(&env),
                0,
            )
            .expect("recorded");
            record_force_majeure(
                &env,
                &admin,
                &outage,
                EventKind::Outage,
                1_000,
                2_000,
                reason(&env),
                0,
            )
        });
        assert_eq!(result.err(), Some(ForceMajeureError::AlreadyRecorded));
    }

    #[test]
    fn closing_after_the_window_succeeds() {
        let env = Env::default();
        let closure = env.as_contract(&contract(&env), || {
            let admin = contract(&env);
            set_governance(&env, &admin);
            let outage = Symbol::new(&env, "OUT1");
            record_force_majeure(
                &env,
                &admin,
                &outage,
                EventKind::Outage,
                1_000,
                5_000,
                reason(&env),
                0,
            )
            .expect("recorded");
            close_force_majeure(&env, &admin, &outage, 6_000)
        });
        assert_eq!(closure, Ok(5_000));
    }

    #[test]
    fn closing_while_the_event_is_running_is_rejected() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let admin = contract(&env);
            set_governance(&env, &admin);
            let outage = Symbol::new(&env, "OUT1");
            record_force_majeure(
                &env,
                &admin,
                &outage,
                EventKind::Outage,
                1_000,
                5_000,
                reason(&env),
                0,
            )
            .expect("recorded");
            close_force_majeure(&env, &admin, &outage, 4_000)
        });
        assert_eq!(result.err(), Some(ForceMajeureError::StillRunning));
    }

    #[test]
    fn closing_an_unrecorded_event_is_rejected() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let admin = contract(&env);
            set_governance(&env, &admin);
            let outage = Symbol::new(&env, "OUT1");
            close_force_majeure(&env, &admin, &outage, 6_000)
        });
        assert_eq!(result.err(), Some(ForceMajeureError::NotRecorded));
    }

    #[test]
    fn a_closed_record_cannot_be_closed_again() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let admin = contract(&env);
            set_governance(&env, &admin);
            let outage = Symbol::new(&env, "OUT1");
            record_force_majeure(
                &env,
                &admin,
                &outage,
                EventKind::Outage,
                1_000,
                5_000,
                reason(&env),
                0,
            )
            .expect("recorded");
            close_force_majeure(&env, &admin, &outage, 6_000).expect("closed");
            close_force_majeure(&env, &admin, &outage, 7_000)
        });
        assert_eq!(result.err(), Some(ForceMajeureError::AlreadyClosed));
    }

    #[test]
    fn the_excused_window_never_exceeds_the_grant() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let admin = contract(&env);
            set_governance(&env, &admin);
            let outage = Symbol::new(&env, "OUT1");
            record_force_majeure(
                &env,
                &admin,
                &outage,
                EventKind::Outage,
                1_000,
                5_000,
                reason(&env),
                0,
            )
            .expect("recorded");
            assert_eq!(excused_seconds(&env, &outage, 99_999), 4_000);
        });
    }

    #[test]
    fn an_unrecorded_event_exempts_nothing() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let outage = Symbol::new(&env, "OUT404");
            assert!(!is_excused(&env, &outage, 1_000));
            assert_eq!(excused_seconds(&env, &outage, 9_999), 0);
            assert_eq!(apply_penalty_bps(&env, &outage, 5_000, 1_000), 5_000);
        });
    }
}
