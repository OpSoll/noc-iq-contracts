// Automatic anomaly detection pause trigger. Complements emergency.rs.
use soroban_sdk::{symbol_short, Env, Symbol};

const ANOMALY_EVENT: Symbol = symbol_short!("anompaus");
const FOUR_HOURS_SECS: u64 = 4 * 60 * 60;

/// If hourly outage volume exceeds 10x the 24h moving-average baseline,
/// returns the timestamp the auto-triggered pause should expire (4h out)
/// and emits AnomalyPauseTriggered.
pub fn check_and_trigger_anomaly_pause(
    env: &Env,
    hourly_volume: u32,
    baseline_24h_avg: u32,
) -> Option<u64> {
    if baseline_24h_avg > 0 && hourly_volume > baseline_24h_avg * 10 {
        env.events().publish((ANOMALY_EVENT,), hourly_volume);
        Some(env.ledger().timestamp() + FOUR_HOURS_SECS)
    } else {
        None
    }
}
