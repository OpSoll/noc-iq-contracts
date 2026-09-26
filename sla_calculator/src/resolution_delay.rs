use soroban_sdk::{contracttype, symbol_short, Env, Map, Symbol};

pub const RESOLVED: Symbol = symbol_short!("resolved");
pub const DETECTED: Symbol = symbol_short!("detected");
pub const DELAY_S: Symbol = symbol_short!("delay_s");
pub const STATE_MAP: Symbol = symbol_short!("r_state");
pub const RECORD_MAP: Symbol = symbol_short!("r_rec");
pub const MAX_DELAY_S: u64 = 86_400;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
pub enum ResolutionState {
    Unknown,
    Pending,
    Resolved,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct ResolutionRecord {
    pub outage_id: Symbol,
    pub detected_at: u64,
    pub resolved_at: u64,
    pub delay_seconds: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DelayError {
    NotDetected,
    AlreadyResolved,
    SequenceRegressed,
    DelayTooLarge,
}

pub fn state_map(env: &Env) -> Map<Symbol, ResolutionState> {
    env.storage()
        .persistent()
        .get(&STATE_MAP)
        .unwrap_or_else(|| Map::new(env))
}

pub fn record_map(env: &Env) -> Map<Symbol, ResolutionRecord> {
    env.storage()
        .persistent()
        .get(&RECORD_MAP)
        .unwrap_or_else(|| Map::new(env))
}

pub fn resolution_state(env: &Env, outage_id: &Symbol) -> ResolutionState {
    state_map(env)
        .get(outage_id.clone())
        .unwrap_or(ResolutionState::Unknown)
}

pub fn resolution_record(env: &Env, outage_id: &Symbol) -> Option<ResolutionRecord> {
    record_map(env).get(outage_id.clone())
}

pub fn mark_detected(env: &Env, outage_id: &Symbol, detected_at: u64) -> Result<(), DelayError> {
    match resolution_state(env, outage_id) {
        ResolutionState::Resolved => return Err(DelayError::AlreadyResolved),
        ResolutionState::Pending => return Ok(()),
        ResolutionState::Unknown => {}
    }
    let mut states = state_map(env);
    states.set(outage_id.clone(), ResolutionState::Pending);
    let mut records = record_map(env);
    records.set(
        outage_id.clone(),
        ResolutionRecord {
            outage_id: outage_id.clone(),
            detected_at,
            resolved_at: 0,
            delay_seconds: 0,
        },
    );
    env.storage().persistent().set(&STATE_MAP, &states);
    env.storage().persistent().set(&RECORD_MAP, &records);
    env.events()
        .publish((DETECTED, outage_id.clone()), detected_at);
    Ok(())
}

pub fn mark_resolved(env: &Env, outage_id: &Symbol, resolved_at: u64) -> Result<u64, DelayError> {
    if resolution_state(env, outage_id) != ResolutionState::Pending {
        return Err(
            if resolution_state(env, outage_id) == ResolutionState::Resolved {
                DelayError::AlreadyResolved
            } else {
                DelayError::NotDetected
            },
        );
    }
    let record = resolution_record(env, outage_id).ok_or(DelayError::NotDetected)?;
    if resolved_at < record.detected_at {
        return Err(DelayError::SequenceRegressed);
    }
    let delay_seconds = resolved_at - record.detected_at;
    if delay_seconds > MAX_DELAY_S {
        return Err(DelayError::DelayTooLarge);
    }
    let stored = ResolutionRecord {
        outage_id: outage_id.clone(),
        detected_at: record.detected_at,
        resolved_at,
        delay_seconds,
    };
    let mut records = record_map(env);
    records.set(outage_id.clone(), stored.clone());
    let mut states = state_map(env);
    states.set(outage_id.clone(), ResolutionState::Resolved);
    env.storage().persistent().set(&RECORD_MAP, &records);
    env.storage().persistent().set(&STATE_MAP, &states);
    env.events()
        .publish((RESOLVED, outage_id.clone()), delay_seconds);
    env.events().publish((DELAY_S, outage_id.clone()), stored);
    Ok(delay_seconds)
}

pub fn resolution_delay_seconds(env: &Env, outage_id: &Symbol) -> Option<u64> {
    resolution_record(env, outage_id).map(|record| record.delay_seconds)
}

pub fn pending_delay(env: &Env, outage_id: &Symbol, now: u64) -> Option<u64> {
    if resolution_state(env, outage_id) != ResolutionState::Pending {
        return None;
    }
    let record = resolution_record(env, outage_id)?;
    if now <= record.detected_at {
        return Some(0);
    }
    let elapsed = now - record.detected_at;
    if elapsed > MAX_DELAY_S {
        Some(MAX_DELAY_S)
    } else {
        Some(elapsed)
    }
}

pub fn has_resolved(env: &Env, outage_id: &Symbol) -> bool {
    resolution_state(env, outage_id) == ResolutionState::Resolved
}

pub fn is_pending(env: &Env, outage_id: &Symbol) -> bool {
    resolution_state(env, outage_id) == ResolutionState::Pending
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SLACalculatorContract;
    use soroban_sdk::testutils::Events as _;

    fn contract(env: &Env) -> soroban_sdk::Address {
        env.register_contract(None, SLACalculatorContract)
    }

    fn outage(env: &Env, text: &str) -> Symbol {
        Symbol::new(env, text)
    }

    #[test]
    fn an_untracked_outage_is_unknown() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let id = outage(&env, "OUT1");
            assert_eq!(resolution_state(&env, &id), ResolutionState::Unknown);
            assert!(resolution_delay_seconds(&env, &id).is_none());
        });
    }

    #[test]
    fn detection_moves_an_outage_to_pending() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let id = outage(&env, "OUT1");
            mark_detected(&env, &id, 1_000).expect("detected");
            assert!(is_pending(&env, &id));
            assert_eq!(pending_delay(&env, &id, 1_000), Some(0));
        });
    }

    #[test]
    fn resolving_reports_the_delay_in_seconds() {
        let env = Env::default();
        let delay = env.as_contract(&contract(&env), || {
            let id = outage(&env, "OUT1");
            mark_detected(&env, &id, 1_000).expect("detected");
            mark_resolved(&env, &id, 1_900)
        });
        assert_eq!(delay, Ok(900));
    }

    #[test]
    fn an_immediate_resolution_has_no_delay() {
        let env = Env::default();
        let delay = env.as_contract(&contract(&env), || {
            let id = outage(&env, "OUT1");
            mark_detected(&env, &id, 1_000).expect("detected");
            mark_resolved(&env, &id, 1_000)
        });
        assert_eq!(delay, Ok(0));
    }

    #[test]
    fn resolving_without_detection_is_rejected() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let id = outage(&env, "OUT1");
            mark_resolved(&env, &id, 1_000)
        });
        assert_eq!(result.err(), Some(DelayError::NotDetected));
    }

    #[test]
    fn resolving_twice_is_rejected() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let id = outage(&env, "OUT1");
            mark_detected(&env, &id, 1_000).expect("detected");
            mark_resolved(&env, &id, 1_100).expect("resolved");
            mark_resolved(&env, &id, 1_200)
        });
        assert_eq!(result.err(), Some(DelayError::AlreadyResolved));
    }

    #[test]
    fn a_resolved_outage_cannot_be_reopened() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let id = outage(&env, "OUT1");
            mark_detected(&env, &id, 1_000).expect("detected");
            mark_resolved(&env, &id, 1_100).expect("resolved");
            mark_detected(&env, &id, 1_200)
        });
        assert_eq!(result.err(), Some(DelayError::AlreadyResolved));
    }

    #[test]
    fn a_backwards_clock_is_rejected() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let id = outage(&env, "OUT1");
            mark_detected(&env, &id, 5_000).expect("detected");
            mark_resolved(&env, &id, 4_000)
        });
        assert_eq!(result.err(), Some(DelayError::SequenceRegressed));
    }

    #[test]
    fn an_absurd_delay_is_rejected() {
        let env = Env::default();
        let result = env.as_contract(&contract(&env), || {
            let id = outage(&env, "OUT1");
            mark_detected(&env, &id, 1_000).expect("detected");
            mark_resolved(&env, &id, 1_000 + MAX_DELAY_S + 1)
        });
        assert_eq!(result.err(), Some(DelayError::DelayTooLarge));
    }

    #[test]
    fn the_full_day_delay_is_accepted() {
        let env = Env::default();
        let delay = env.as_contract(&contract(&env), || {
            let id = outage(&env, "OUT1");
            mark_detected(&env, &id, 1_000).expect("detected");
            mark_resolved(&env, &id, 1_000 + MAX_DELAY_S)
        });
        assert_eq!(delay, Ok(MAX_DELAY_S));
    }

    #[test]
    fn repeated_detection_is_idempotent() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let id = outage(&env, "OUT1");
            mark_detected(&env, &id, 1_000).expect("detected");
            mark_detected(&env, &id, 1_200).expect("detected again");
            assert!(is_pending(&env, &id));
        });
    }

    #[test]
    fn detection_publishes_exactly_one_event() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let id = outage(&env, "OUT1");
            mark_detected(&env, &id, 1_000).expect("detected");
            mark_detected(&env, &id, 1_200).expect("detected again");
            assert_eq!(env.events().all().len(), 1);
        });
    }

    #[test]
    fn resolution_publishes_both_events() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let id = outage(&env, "OUT1");
            mark_detected(&env, &id, 1_000).expect("detected");
            mark_resolved(&env, &id, 1_300).expect("resolved");
            assert_eq!(env.events().all().len(), 3);
        });
    }

    #[test]
    fn a_pending_outage_reports_its_age() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let id = outage(&env, "OUT1");
            mark_detected(&env, &id, 1_000).expect("detected");
            assert_eq!(pending_delay(&env, &id, 4_600), Some(3_600));
        });
    }

    #[test]
    fn a_resolved_outage_no_longer_ages() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let id = outage(&env, "OUT1");
            mark_detected(&env, &id, 1_000).expect("detected");
            mark_resolved(&env, &id, 1_100).expect("resolved");
            assert!(has_resolved(&env, &id));
            assert!(pending_delay(&env, &id, 9_999).is_none());
        });
    }

    #[test]
    fn outages_are_tracked_independently() {
        let env = Env::default();
        env.as_contract(&contract(&env), || {
            let first = outage(&env, "OUT1");
            let second = outage(&env, "OUT2");
            mark_detected(&env, &first, 1_000).expect("first");
            mark_resolved(&env, &first, 1_100).expect("resolved");
            mark_detected(&env, &second, 2_000).expect("second");
            assert!(has_resolved(&env, &first));
            assert!(is_pending(&env, &second));
            assert_eq!(resolution_delay_seconds(&env, &first), Some(100));
        });
    }
}
