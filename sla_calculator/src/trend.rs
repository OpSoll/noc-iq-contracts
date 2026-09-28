use soroban_sdk::{symbol_short, Address, Env, Symbol};

use crate::{SLAError, SLAResult, HISTORY_KEY};

// -----------------------------------------------------------------------
// Issue #699: operator SLA performance tier badges
// -----------------------------------------------------------------------

/// Rolling compliance window used for tier assignment.
const TIER_WINDOW_SECS: u64 = 90 * 24 * 60 * 60;

/// Storage key for per-operator outcome history feeding tier assignment.
const OPERATOR_OUTCOMES_KEY: Symbol = symbol_short!("OP_HIST");

/// Gold/Silver/Bronze thresholds, in basis points of the rolling
/// compliance score (10_000 = 100%).
const GOLD_THRESHOLD_BPS: u32 = 9990; // > 99.9%
const SILVER_THRESHOLD_BPS: u32 = 9900; // > 99.0%
const BRONZE_THRESHOLD_BPS: u32 = 9500; // > 95.0%

/// A single recorded SLA outcome for an operator, used to compute their
/// rolling compliance score.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperatorOutcome {
    /// Ledger timestamp the outcome was recorded at.
    pub recorded_at: u64,
    /// Whether the SLA was met (`true`) or violated (`false`).
    pub met: bool,
}

/// Issue #699: operator SLA performance tier badge.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OperatorTier {
    /// Rolling 90-day compliance > 99.9%.
    Gold,
    /// Rolling 90-day compliance > 99.0%.
    Silver,
    /// Rolling 90-day compliance > 95.0%.
    Bronze,
    /// Rolling 90-day compliance <= 95.0% (or no history yet).
    AtRisk,
}

// -----------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------

/// SLA trend data for a specific time period.
#[soroban_sdk::contracttype]
pub struct TrendData {
    /// Start timestamp of the period.
    pub period_start: u64,
    /// End timestamp of the period.
    pub period_end: u64,
    /// Number of calculations in this period.
    pub calculation_count: u32,
    /// Number of SLA violations.
    pub violation_count: u32,
    /// SLA compliance rate (0-10000 basis points, where 10000 = 100%).
    pub compliance_rate_bps: u32,
    /// Average MTTR in minutes.
    pub avg_mttr_minutes: u32,
    /// Total rewards distributed.
    pub total_rewards: i128,
    /// Total penalties assessed.
    pub total_penalties: i128,
    /// Net payment amount (rewards - penalties).
    pub net_amount: i128,
}

/// Aggregated trend summary across multiple periods.
#[soroban_sdk::contracttype]
pub struct TrendSummary {
    /// Overall compliance rate across all periods.
    pub overall_compliance_bps: u32,
    /// Overall average MTTR.
    pub overall_avg_mttr: u32,
    /// Total calculations across all periods.
    pub total_calculations: u32,
    /// Trend direction: positive = improving, negative = degrading.
    pub trend_direction: i32,
    /// Number of periods analyzed.
    pub period_count: u32,
}

// -----------------------------------------------------------------------
// Functions
// -----------------------------------------------------------------------

/// Calculate SLA trend for a specific time window.
///
/// Returns aggregated metrics for all calculations within the time range.
///
/// # Arguments
/// - `from_timestamp`: Start of analysis window (inclusive).
/// - `to_timestamp`: End of analysis window (inclusive).
///
/// # Returns
/// TrendData with aggregated metrics for the period.
pub fn calculate_trend(
    env: &Env,
    from_timestamp: u64,
    to_timestamp: u64,
) -> Result<TrendData, SLAError> {
    let history: soroban_sdk::Vec<SLAResult> = env
        .storage()
        .instance()
        .get(&HISTORY_KEY)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));

    let mut calculation_count: u32 = 0;
    let mut violation_count: u32 = 0;
    let mut total_mttr: u64 = 0;
    let mut total_rewards: i128 = 0;
    let mut total_penalties: i128 = 0;

    for i in 0..history.len() {
        let entry = history.get(i).unwrap();

        // Filter by time range
        if entry.recorded_at < from_timestamp || entry.recorded_at > to_timestamp {
            continue;
        }

        calculation_count = calculation_count.saturating_add(1);
        total_mttr = total_mttr.saturating_add(entry.mttr_minutes as u64);

        if entry.status == symbol_short!("viol") {
            violation_count = violation_count.saturating_add(1);
            total_penalties = total_penalties.saturating_add(entry.amount);
        } else {
            total_rewards = total_rewards.saturating_add(entry.amount);
        }
    }

    // Calculate compliance rate (basis points)
    let compliance_rate_bps = if calculation_count > 0 {
        let met_count = calculation_count - violation_count;
        (met_count as u64 * 10000 / calculation_count as u64) as u32
    } else {
        0
    };

    // Calculate average MTTR
    let avg_mttr_minutes = if calculation_count > 0 {
        (total_mttr / calculation_count as u64) as u32
    } else {
        0
    };

    Ok(TrendData {
        period_start: from_timestamp,
        period_end: to_timestamp,
        calculation_count,
        violation_count,
        compliance_rate_bps,
        avg_mttr_minutes,
        total_rewards,
        total_penalties,
        net_amount: total_rewards - total_penalties,
    })
}

/// Calculate trend across multiple time buckets.
///
/// Divides the time range into equal buckets and calculates metrics
/// for each bucket to identify trends over time.
///
/// # Arguments
/// - `from_timestamp`: Start of analysis window.
/// - `to_timestamp`: End of analysis window.
/// - `bucket_count`: Number of time buckets to divide into.
///
/// # Returns
/// TrendSummary with overall metrics and trend direction.
pub fn calculate_trend_summary(
    env: &Env,
    from_timestamp: u64,
    to_timestamp: u64,
    bucket_count: u32,
) -> Result<TrendSummary, SLAError> {
    if bucket_count == 0 {
        return Err(SLAError::InvalidThreshold);
    }

    let duration = to_timestamp.saturating_sub(from_timestamp);
    let bucket_duration = duration / bucket_count as u64;

    let mut total_calculations: u32 = 0;
    let mut total_compliance: u64 = 0;
    let mut total_mttr: u64 = 0;
    let mut total_calculations_for_mttr: u32 = 0;
    let mut prev_compliance: i64 = -1;
    let mut trend_sum: i64 = 0;

    for i in 0..bucket_count {
        let bucket_start = from_timestamp + (i as u64 * bucket_duration);
        let bucket_end = if i == bucket_count - 1 {
            to_timestamp
        } else {
            bucket_start + bucket_duration
        };

        let trend = calculate_trend(env, bucket_start, bucket_end)?;

        total_calculations = total_calculations.saturating_add(trend.calculation_count);
        total_compliance = total_compliance.saturating_add(trend.compliance_rate_bps as u64);

        if trend.calculation_count > 0 {
            total_mttr = total_mttr
                .saturating_add(trend.avg_mttr_minutes as u64 * trend.calculation_count as u64);
            total_calculations_for_mttr =
                total_calculations_for_mttr.saturating_add(trend.calculation_count);
        }

        // Calculate trend direction (positive = improving compliance)
        if prev_compliance >= 0 {
            let diff = trend.compliance_rate_bps as i64 - prev_compliance;
            trend_sum = trend_sum.saturating_add(diff);
        }
        prev_compliance = trend.compliance_rate_bps as i64;
    }

    let overall_compliance_bps = if bucket_count > 0 {
        (total_compliance / bucket_count as u64) as u32
    } else {
        0
    };

    let overall_avg_mttr = if total_calculations_for_mttr > 0 {
        (total_mttr / total_calculations_for_mttr as u64) as u32
    } else {
        0
    };

    Ok(TrendSummary {
        overall_compliance_bps,
        overall_avg_mttr,
        total_calculations,
        trend_direction: trend_sum as i32,
        period_count: bucket_count,
    })
}

/// Get the most recent trend data (last N calculations).
pub fn get_recent_trend(env: &Env, lookback_count: u32) -> Result<TrendData, SLAError> {
    let history: soroban_sdk::Vec<SLAResult> = env
        .storage()
        .instance()
        .get(&HISTORY_KEY)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));

    let len = history.len();
    if len == 0 {
        return Ok(TrendData {
            period_start: 0,
            period_end: 0,
            calculation_count: 0,
            violation_count: 0,
            compliance_rate_bps: 0,
            avg_mttr_minutes: 0,
            total_rewards: 0,
            total_penalties: 0,
            net_amount: 0,
        });
    }

    let start_idx = len.saturating_sub(lookback_count);
    let from = history.get(start_idx).unwrap().recorded_at;
    let to = history.get(len - 1).unwrap().recorded_at;

    calculate_trend(env, from, to)
}

/// Record an SLA outcome for `operator`, feeding the rolling 90-day
/// compliance score used by `get_operator_tier`.
pub fn record_operator_outcome(env: &Env, operator: &Address, met: bool) -> Result<(), SLAError> {
    let mut all: soroban_sdk::Map<Address, soroban_sdk::Vec<OperatorOutcome>> = env
        .storage()
        .instance()
        .get(&OPERATOR_OUTCOMES_KEY)
        .unwrap_or_else(|| soroban_sdk::Map::new(env));

    let mut history = all
        .get(operator.clone())
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));
    history.push_back(OperatorOutcome {
        recorded_at: env.ledger().timestamp(),
        met,
    });

    all.set(operator.clone(), history);
    env.storage().instance().set(&OPERATOR_OUTCOMES_KEY, &all);

    Ok(())
}

/// Issue #699: calculates `operator`'s rolling 90-day SLA compliance
/// score, in basis points (10_000 = 100%), from outcomes recorded via
/// `record_operator_outcome`. Outcomes older than `TIER_WINDOW_SECS`
/// relative to the current ledger timestamp are excluded.
///
/// Returns `0` if the operator has no outcomes within the window.
pub fn calculate_operator_compliance_bps(env: &Env, operator: &Address) -> u32 {
    let all: soroban_sdk::Map<Address, soroban_sdk::Vec<OperatorOutcome>> = env
        .storage()
        .instance()
        .get(&OPERATOR_OUTCOMES_KEY)
        .unwrap_or_else(|| soroban_sdk::Map::new(env));

    let history = match all.get(operator.clone()) {
        Some(h) => h,
        None => return 0,
    };

    let now = env.ledger().timestamp();
    let window_start = now.saturating_sub(TIER_WINDOW_SECS);

    let mut total: u32 = 0;
    let mut met_count: u32 = 0;
    for outcome in history.iter() {
        if outcome.recorded_at < window_start {
            continue;
        }
        total = total.saturating_add(1);
        if outcome.met {
            met_count = met_count.saturating_add(1);
        }
    }

    if total == 0 {
        0
    } else {
        (met_count as u64 * 10_000 / total as u64) as u32
    }
}

/// Issue #699 acceptance criterion: `get_operator_tier` getter — maps
/// `operator`'s rolling 90-day compliance score to a `Gold` / `Silver` /
/// `Bronze` / `AtRisk` badge.
pub fn get_operator_tier(env: &Env, operator: &Address) -> OperatorTier {
    let compliance_bps = calculate_operator_compliance_bps(env, operator);

    if compliance_bps > GOLD_THRESHOLD_BPS {
        OperatorTier::Gold
    } else if compliance_bps > SILVER_THRESHOLD_BPS {
        OperatorTier::Silver
    } else if compliance_bps > BRONZE_THRESHOLD_BPS {
        OperatorTier::Bronze
    } else {
        OperatorTier::AtRisk
    }
}

// -----------------------------------------------------------------------
// Issue #665: rolling 30-day cumulative uptime window
// -----------------------------------------------------------------------

/// Length of the rolling SLA window — 30 days — in seconds.
pub const ROLLING_WINDOW_SECS: u64 = 30 * 24 * 60 * 60;

/// Storage key for the rolling downtime ledger.
const ROLLING_DOWNTIME_KEY: Symbol = symbol_short!("ROLL30");

/// A single closed outage, recorded against the rolling window.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RollingDowntimeEntry {
    /// Ledger timestamp at which the outage closed.
    pub closed_at: u64,
    /// Outage duration in seconds.
    pub downtime_seconds: u64,
}

/// Lightweight snapshot of the rolling 30-day availability window.
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RollingUptimeWindow {
    /// Start of the evaluated window (inclusive).
    pub window_start: u64,
    /// End of the evaluated window (the current ledger timestamp).
    pub window_end: u64,
    /// Cumulative downtime inside the window, in seconds.
    pub downtime_seconds: u64,
    /// Number of outages still inside the window.
    pub outage_count: u32,
    /// Availability inside the window, in basis points (`10_000` == 100%).
    pub availability_bps: u32,
}

fn load_rolling_entries(env: &Env) -> soroban_sdk::Vec<RollingDowntimeEntry> {
    env.storage()
        .instance()
        .get(&ROLLING_DOWNTIME_KEY)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env))
}

/// Availability in basis points for a 30-day window carrying
/// `downtime_seconds` of downtime. Downtime is clamped to the window length.
fn rolling_availability_bps(downtime_seconds: u64) -> u32 {
    let downtime = downtime_seconds.min(ROLLING_WINDOW_SECS);
    let uptime = ROLLING_WINDOW_SECS - downtime;
    (uptime as u128 * 10_000u128 / ROLLING_WINDOW_SECS as u128) as u32
}

/// Collects the entries that are still inside the rolling window, returning the
/// surviving entries alongside the cumulative downtime and outage count.
///
/// This is a pure read: callers that need to persist the eviction result (i.e.
/// `record_outage_close`) write the returned vector themselves.
fn collect_window(env: &Env, now: u64) -> (soroban_sdk::Vec<RollingDowntimeEntry>, u64, u32) {
    let window_start = now.saturating_sub(ROLLING_WINDOW_SECS);
    let entries = load_rolling_entries(env);

    let mut kept = soroban_sdk::Vec::new(env);
    let mut downtime: u64 = 0;
    let mut count: u32 = 0;

    for entry in entries.iter() {
        if entry.closed_at < window_start {
            // Evicted: the outage fell out of the 30-day window.
            continue;
        }
        downtime = downtime.saturating_add(entry.downtime_seconds);
        count = count.saturating_add(1);
        kept.push_back(entry);
    }

    (kept, downtime, count)
}

/// Records a closed outage against the rolling 30-day window.
///
/// The cumulative downtime counter is updated and every entry that has fallen
/// outside the window is evicted in the same storage write, so the rolling
/// state stays bounded no matter how many outages accumulate.
///
/// Returns the refreshed [`RollingUptimeWindow`]. An `outage_end` that precedes
/// `outage_start` surfaces as [`SLAError::InvalidTimestampSequence`].
pub fn record_outage_close(
    env: &Env,
    outage_start: u64,
    outage_end: u64,
) -> Result<RollingUptimeWindow, SLAError> {
    if outage_end < outage_start {
        return Err(SLAError::InvalidTimestampSequence);
    }

    let now = env.ledger().timestamp();
    let window_start = now.saturating_sub(ROLLING_WINDOW_SECS);
    let (mut kept, mut downtime, _) = collect_window(env, now);

    let duration = outage_end.saturating_sub(outage_start);
    if outage_end >= window_start {
        downtime = downtime.saturating_add(duration);
        kept.push_back(RollingDowntimeEntry {
            closed_at: outage_end,
            downtime_seconds: duration,
        });
    }

    // A single atomic write keeps the persisted ledger and the returned
    // snapshot consistent.
    env.storage().instance().set(&ROLLING_DOWNTIME_KEY, &kept);

    Ok(RollingUptimeWindow {
        window_start,
        window_end: now,
        downtime_seconds: downtime,
        outage_count: kept.len(),
        availability_bps: rolling_availability_bps(downtime),
    })
}

/// Lightweight getter for the active rolling 30-day SLA percentage, in basis
/// points (`10_000` == 100%).
///
/// Entries that have fallen outside the window are excluded from the returned
/// figure. This is a read-only query and never mutates storage.
pub fn get_rolling_sla_bps(env: &Env) -> u32 {
    let now = env.ledger().timestamp();
    let (_, downtime, _) = collect_window(env, now);
    rolling_availability_bps(downtime)
}

/// Full snapshot of the rolling window, including the cumulative downtime
/// counter and the number of contributing outages.
///
/// Read-only: expired entries are ignored but only `record_outage_close`
/// rewrites the stored ledger.
pub fn get_rolling_uptime_window(env: &Env) -> RollingUptimeWindow {
    let now = env.ledger().timestamp();
    let (_, downtime, count) = collect_window(env, now);
    RollingUptimeWindow {
        window_start: now.saturating_sub(ROLLING_WINDOW_SECS),
        window_end: now,
        downtime_seconds: downtime,
        outage_count: count,
        availability_bps: rolling_availability_bps(downtime),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger};
    use soroban_sdk::Address;

    fn record_n(env: &Env, operator: &Address, met_count: u32, violation_count: u32) {
        for _ in 0..met_count {
            record_operator_outcome(env, operator, true).unwrap();
        }
        for _ in 0..violation_count {
            record_operator_outcome(env, operator, false).unwrap();
        }
    }

    #[test]
    fn test_operator_with_no_history_is_at_risk() {
        let env = Env::default();
        let cid = env.register_contract(None, crate::SLACalculatorContract);
        let operator = Address::generate(&env);

        env.as_contract(&cid, || {
            assert_eq!(get_operator_tier(&env, &operator), OperatorTier::AtRisk);
        });
    }

    #[test]
    fn test_tier_boundaries() {
        let env = Env::default();
        let cid = env.register_contract(None, crate::SLACalculatorContract);

        // Gold: > 99.9% — 1000 met, 0 violations = 100%.
        let gold_op = Address::generate(&env);
        env.as_contract(&cid, || {
            record_n(&env, &gold_op, 1000, 0);
            assert_eq!(get_operator_tier(&env, &gold_op), OperatorTier::Gold);
        });

        // Silver: > 99.0% and <= 99.9% — 995 met, 5 violated = 99.5%.
        let silver_op = Address::generate(&env);
        env.as_contract(&cid, || {
            record_n(&env, &silver_op, 995, 5);
            assert_eq!(get_operator_tier(&env, &silver_op), OperatorTier::Silver);
        });

        // Bronze: > 95.0% and <= 99.0% — 97 met, 3 violated = 97%.
        let bronze_op = Address::generate(&env);
        env.as_contract(&cid, || {
            record_n(&env, &bronze_op, 97, 3);
            assert_eq!(get_operator_tier(&env, &bronze_op), OperatorTier::Bronze);
        });

        // AtRisk: <= 95.0% — 90 met, 10 violated = 90%.
        let at_risk_op = Address::generate(&env);
        env.as_contract(&cid, || {
            record_n(&env, &at_risk_op, 90, 10);
            assert_eq!(get_operator_tier(&env, &at_risk_op), OperatorTier::AtRisk);
        });
    }

    #[test]
    fn test_outcomes_outside_90_day_window_are_excluded() {
        let env = Env::default();
        let cid = env.register_contract(None, crate::SLACalculatorContract);
        let operator = Address::generate(&env);

        env.ledger().set_timestamp(1_000);
        env.as_contract(&cid, || {
            // All violations, far in the past.
            record_n(&env, &operator, 0, 50);
        });

        // Jump forward past the 90-day window and record a clean record.
        env.ledger().set_timestamp(1_000 + TIER_WINDOW_SECS + 1);
        env.as_contract(&cid, || {
            record_n(&env, &operator, 10, 0);
            // Only the 10 recent "met" outcomes are in-window now.
            assert_eq!(get_operator_tier(&env, &operator), OperatorTier::Gold);
        });
    }

    #[test]
    fn test_operator_histories_are_independent() {
        let env = Env::default();
        let cid = env.register_contract(None, crate::SLACalculatorContract);
        let op_a = Address::generate(&env);
        let op_b = Address::generate(&env);

        env.as_contract(&cid, || {
            record_n(&env, &op_a, 1000, 0);
            record_n(&env, &op_b, 0, 1000);

            assert_eq!(get_operator_tier(&env, &op_a), OperatorTier::Gold);
            assert_eq!(get_operator_tier(&env, &op_b), OperatorTier::AtRisk);
        });
    }
}

#[cfg(test)]
mod rolling_window_tests {
    use super::*;
    use soroban_sdk::testutils::Ledger;

    const DAY: u64 = 24 * 60 * 60;

    fn with_contract<R>(env: &Env, f: impl FnOnce() -> R) -> R {
        let contract_id = env.register_contract(None, crate::SLACalculatorContract);
        env.as_contract(&contract_id, f)
    }

    #[test]
    fn an_empty_window_reports_full_availability() {
        let env = Env::default();
        env.ledger().set_timestamp(10 * DAY);

        with_contract(&env, || {
            assert_eq!(get_rolling_sla_bps(&env), 10_000);

            let window = get_rolling_uptime_window(&env);
            assert_eq!(window.downtime_seconds, 0);
            assert_eq!(window.outage_count, 0);
            assert_eq!(window.availability_bps, 10_000);
            assert_eq!(window.window_end, 10 * DAY);
            // The window is clamped at the epoch, so it cannot start negative.
            assert_eq!(window.window_start, 0);
        });
    }

    #[test]
    fn a_closed_outage_reduces_the_rolling_availability() {
        let env = Env::default();
        env.ledger().set_timestamp(10 * DAY);

        with_contract(&env, || {
            let window = record_outage_close(&env, 10 * DAY - 3_600, 10 * DAY).unwrap();

            assert_eq!(window.downtime_seconds, 3_600);
            assert_eq!(window.outage_count, 1);
            // 1 h of downtime inside a 30-day window == 99.86% availability.
            assert_eq!(window.availability_bps, 9_986);
            assert_eq!(get_rolling_sla_bps(&env), 9_986);
        });
    }

    #[test]
    fn outages_accumulate_in_the_rolling_counter() {
        let env = Env::default();
        env.ledger().set_timestamp(5 * DAY);

        with_contract(&env, || {
            record_outage_close(&env, 5 * DAY - 1_200, 5 * DAY - 600).unwrap(); // 10 min
            let window = record_outage_close(&env, 5 * DAY - 600, 5 * DAY).unwrap(); // 10 min

            assert_eq!(window.downtime_seconds, 1_200);
            assert_eq!(window.outage_count, 2);
        });
    }

    #[test]
    fn entries_older_than_the_window_are_evicted_on_write() {
        let env = Env::default();
        let t0 = 40 * DAY;
        env.ledger().set_timestamp(t0);

        with_contract(&env, || {
            record_outage_close(&env, t0 - 3_600, t0).unwrap();

            let fresh = get_rolling_uptime_window(&env);
            assert_eq!(fresh.downtime_seconds, 3_600);
            // Past the window length, the full 30-day span is exposed.
            assert_eq!(fresh.window_end - fresh.window_start, ROLLING_WINDOW_SECS);

            // Move past the 30-day horizon: the first outage is now stale.
            let t1 = t0 + ROLLING_WINDOW_SECS + 1;
            env.ledger().set_timestamp(t1);
            let window = record_outage_close(&env, t1 - 60, t1).unwrap();

            assert_eq!(
                window.downtime_seconds, 60,
                "stale downtime must be evicted from the rolling counter"
            );
            assert_eq!(window.outage_count, 1);
        });
    }

    #[test]
    fn the_getter_ignores_expired_entries_without_rewriting_state() {
        let env = Env::default();
        let t0 = 40 * DAY;
        env.ledger().set_timestamp(t0);

        with_contract(&env, || {
            record_outage_close(&env, t0 - 3_600, t0).unwrap();

            env.ledger().set_timestamp(t0 + ROLLING_WINDOW_SECS + 1);
            let window = get_rolling_uptime_window(&env);

            assert_eq!(window.downtime_seconds, 0);
            assert_eq!(window.outage_count, 0);
            assert_eq!(get_rolling_sla_bps(&env), 10_000);
        });
    }

    #[test]
    fn downtime_beyond_the_window_floors_availability_at_zero() {
        let env = Env::default();
        env.ledger().set_timestamp(30 * DAY);

        with_contract(&env, || {
            // 45 days of continuous downtime inside a 30-day window.
            record_outage_close(&env, 0, 45 * DAY).unwrap();
            assert_eq!(get_rolling_sla_bps(&env), 0);
        });
    }

    #[test]
    fn an_event_that_ends_before_it_starts_is_rejected() {
        let env = Env::default();
        env.ledger().set_timestamp(DAY);

        with_contract(&env, || {
            assert_eq!(
                record_outage_close(&env, 500, 100),
                Err(SLAError::InvalidTimestampSequence)
            );
        });
    }
}
