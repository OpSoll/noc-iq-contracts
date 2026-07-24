#![no_std]

use soroban_sdk::{symbol_short, Env, Symbol};

use crate::{SLAResult, SLAError, HISTORY_KEY};

// -----------------------------------------------------------------------
// Storage keys
// -----------------------------------------------------------------------
const FILTER_CONFIG_KEY: Symbol = symbol_short!("FLTCFG");

// -----------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------

/// Event date filter configuration.
#[soroban_sdk::contracttype]
pub struct EventFilter {
    /// Start timestamp (inclusive). None means no lower bound.
    pub from_timestamp: Option<u64>,
    /// End timestamp (inclusive). None means no upper bound.
    pub to_timestamp: Option<u64>,
    /// Filter by status. None means all statuses.
    pub status_filter: Option<soroban_sdk::Symbol>,
    /// Filter by payment type. None means all types.
    pub payment_type_filter: Option<soroban_sdk::Symbol>,
    /// Maximum number of results to return.
    pub limit: u32,
    /// Zero-based offset for pagination.
    pub offset: u32,
}

// -----------------------------------------------------------------------
// Functions
// -----------------------------------------------------------------------

/// Filter history entries by date range and optional status/payment type filters.
///
/// Returns a paginated list of SLA results matching the filter criteria.
/// Results are ordered by `recorded_at` ascending (oldest first).
///
/// # Arguments
/// - `from_timestamp`: Start of time range (inclusive). None for no lower bound.
/// - `to_timestamp`: End of time range (inclusive). None for no upper bound.
/// - `status_filter`: Optional status filter ("met" or "viol"). None for all.
/// - `payment_type_filter`: Optional payment type filter ("rew" or "pen"). None for all.
/// - `offset`: Zero-based start index for pagination.
/// - `limit`: Maximum number of results per page.
///
/// # Returns
/// Filtered and paginated list of SLA results.
pub fn filter_events(
    env: &Env,
    from_timestamp: Option<u64>,
    to_timestamp: Option<u64>,
    status_filter: Option<soroban_sdk::Symbol>,
    payment_type_filter: Option<soroban_sdk::Symbol>,
    offset: u32,
    limit: u32,
) -> Result<soroban_sdk::Vec<SLAResult>, SLAError> {
    let history: soroban_sdk::Vec<SLAResult> = env
        .storage()
        .instance()
        .get(&HISTORY_KEY)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));

    let mut filtered = soroban_sdk::Vec::new(env);
    let mut skipped: u32 = 0;
    let mut collected: u32 = 0;

    for i in 0..history.len() {
        let entry = history.get(i).unwrap();

        // Apply timestamp filters
        if let Some(from) = from_timestamp {
            if entry.recorded_at < from {
                continue;
            }
        }
        if let Some(to) = to_timestamp {
            if entry.recorded_at > to {
                continue;
            }
        }

        // Apply status filter
        if let Some(ref status) = status_filter {
            if entry.status != *status {
                continue;
            }
        }

        // Apply payment type filter
        if let Some(ref payment_type) = payment_type_filter {
            if entry.payment_type != *payment_type {
                continue;
            }
        }

        // Apply pagination
        if skipped < offset {
            skipped += 1;
            continue;
        }

        if collected >= limit {
            break;
        }

        filtered.push_back(entry);
        collected += 1;
    }

    Ok(filtered)
}

/// Count events matching filter criteria (without pagination).
///
/// Useful for displaying total results count in UI.
pub fn count_filtered_events(
    env: &Env,
    from_timestamp: Option<u64>,
    to_timestamp: Option<u64>,
    status_filter: Option<soroban_sdk::Symbol>,
    payment_type_filter: Option<soroban_sdk::Symbol>,
) -> Result<u32, SLAError> {
    let history: soroban_sdk::Vec<SLAResult> = env
        .storage()
        .instance()
        .get(&HISTORY_KEY)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));

    let mut count: u32 = 0;

    for i in 0..history.len() {
        let entry = history.get(i).unwrap();

        if let Some(from) = from_timestamp {
            if entry.recorded_at < from {
                continue;
            }
        }
        if let Some(to) = to_timestamp {
            if entry.recorded_at > to {
                continue;
            }
        }

        if let Some(ref status) = status_filter {
            if entry.status != *status {
                continue;
            }
        }

        if let Some(ref payment_type) = payment_type_filter {
            if entry.payment_type != *payment_type {
                continue;
            }
        }

        count = count.saturating_add(1);
    }

    Ok(count)
}

/// Get events within a time window (convenience wrapper).
///
/// Returns all events between `start_seconds_ago` and `end_seconds_ago`
/// relative to the current ledger timestamp.
pub fn get_events_in_window(
    env: &Env,
    start_seconds_ago: u64,
    end_seconds_ago: u64,
    limit: u32,
) -> Result<soroban_sdk::Vec<SLAResult>, SLAError> {
    let now = env.ledger().timestamp();
    let from = now.saturating_sub(start_seconds_ago);
    let to = now.saturating_sub(end_seconds_ago);

    filter_events(env, Some(from), Some(to), None, None, 0, limit)
}

/// Get violation events only within a time range.
pub fn get_violations_in_range(
    env: &Env,
    from_timestamp: u64,
    to_timestamp: u64,
    limit: u32,
) -> Result<soroban_sdk::Vec<SLAResult>, SLAError> {
    filter_events(
        env,
        Some(from_timestamp),
        Some(to_timestamp),
        Some(symbol_short!("viol")),
        None,
        0,
        limit,
    )
}

/// Get reward events only within a time range.
pub fn get_rewards_in_range(
    env: &Env,
    from_timestamp: u64,
    to_timestamp: u64,
    limit: u32,
) -> Result<soroban_sdk::Vec<SLAResult>, SLAError> {
    filter_events(
        env,
        Some(from_timestamp),
        Some(to_timestamp),
        None,
        Some(symbol_short!("rew")),
        0,
        limit,
    )
}
