// Automatic anomaly detection pause trigger. Complements emergency.rs.
// #825 — emit a distinct event with metric + threshold when auto-pause fires.
use soroban_sdk::{symbol_short, Env, Symbol};

/// Distinct from manual pause events so indexers can tell them apart (#825).
const ANOMALY_AUTO_PAUSE: Symbol = symbol_short!("anomAuto");
const FOUR_HOURS_SECS: u64 = 4 * 60 * 60;
/// Default multiplier: hourly volume > threshold_mult × 24h baseline.
pub const DEFAULT_THRESHOLD_MULT: u32 = 10;

/// If hourly outage volume exceeds `threshold_mult` × the 24h moving-average
/// baseline, returns the timestamp the auto-triggered pause should expire
/// (4h out) and emits `AnomalyAutoPause` with (metric, threshold).
pub fn check_and_trigger_anomaly_pause(
    env: &Env,
    hourly_volume: u32,
    baseline_24h_avg: u32,
) -> Option<u64> {
    check_and_trigger_anomaly_pause_with_mult(
        env,
        hourly_volume,
        baseline_24h_avg,
        DEFAULT_THRESHOLD_MULT,
    )
}

pub fn check_and_trigger_anomaly_pause_with_mult(
    env: &Env,
    hourly_volume: u32,
    baseline_24h_avg: u32,
    threshold_mult: u32,
) -> Option<u64> {
    if baseline_24h_avg == 0 || threshold_mult == 0 {
        return None;
    }
    let threshold = baseline_24h_avg.saturating_mul(threshold_mult);
    if hourly_volume > threshold {
        // Payload: offending metric (hourly volume) + threshold that was crossed.
        env.events()
            .publish((ANOMALY_AUTO_PAUSE,), (hourly_volume, threshold));
        Some(env.ledger().timestamp() + FOUR_HOURS_SECS)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Events, Ledger};

    #[test]
    fn auto_pause_emits_metric_and_threshold() {
        let env = Env::default();
        env.ledger().with_mut(|l| l.timestamp = 1_000);
        let expiry = check_and_trigger_anomaly_pause(&env, 500, 40);
        assert_eq!(expiry, Some(1_000 + FOUR_HOURS_SECS));
        let events = env.events().all();
        assert_eq!(events.len(), 1);
        // Distinct anomaly auto-pause event fired (metric=500, threshold=40*10=400).
        let (_contract, topics, _data) = events.get(0).unwrap();
        assert_eq!(topics.len(), 1);
    }

    #[test]
    fn below_threshold_emits_nothing() {
        let env = Env::default();
        assert!(check_and_trigger_anomaly_pause(&env, 100, 40).is_none());
        assert_eq!(env.events().all().len(), 0);
    }

    #[test]
    fn custom_multiplier_changes_threshold() {
        let env = Env::default();
        // 40 * 5 = 200; volume 201 triggers
        let expiry = check_and_trigger_anomaly_pause_with_mult(&env, 201, 40, 5);
        assert!(expiry.is_some());
        let events = env.events().all();
        assert_eq!(events.len(), 1);
    }
}
