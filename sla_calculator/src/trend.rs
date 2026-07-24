#![no_std]

use soroban_sdk::{symbol_short, Env, Symbol};

use crate::{SLAResult, SLAError, HISTORY_KEY};

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
            total_mttr = total_mttr.saturating_add(
                trend.avg_mttr_minutes as u64 * trend.calculation_count as u64,
            );
            total_calculations_for_mttr = total_calculations_for_mttr
                .saturating_add(trend.calculation_count);
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
pub fn get_recent_trend(
    env: &Env,
    lookback_count: u32,
) -> Result<TrendData, SLAError> {
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
