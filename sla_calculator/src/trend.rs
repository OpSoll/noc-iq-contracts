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
