#![no_std]

use soroban_sdk::{symbol_short, Env, Symbol};

use crate::{SLAResult, SLAError, HISTORY_KEY};

// -----------------------------------------------------------------------
// Storage keys
// -----------------------------------------------------------------------
const EVENT_INDEX_KEY: Symbol = symbol_short!("EIDX");

// -----------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------

/// Event index entry for efficient lookup.
#[soroban_sdk::contracttype]
pub struct EventIndexEntry {
    /// Position in the history vector.
    pub position: u32,
    /// Outage ID for quick lookup.
    pub outage_id: Symbol,
    /// Recorded timestamp for time-based queries.
    pub recorded_at: u64,
    /// Status for status-based queries.
    pub status: Symbol,
    /// Payment type for payment-type queries.
    pub payment_type: Symbol,
}

/// Event index for efficient querying.
#[soroban_sdk::contracttype]
pub struct EventIndex {
    /// All index entries.
    pub entries: soroban_sdk::Vec<EventIndexEntry>,
    /// Total indexed events.
    pub count: u32,
}

// -----------------------------------------------------------------------
// Functions
// -----------------------------------------------------------------------

/// Build or rebuild the event index from history.
///
/// Creates an index for efficient querying by outage_id, timestamp,
/// status, and payment_type.
pub fn build_event_index(env: &Env) -> Result<EventIndex, SLAError> {
    let history: soroban_sdk::Vec<SLAResult> = env
        .storage()
        .instance()
        .get(&HISTORY_KEY)
        .unwrap_or_else(|| soroban_sdk::Vec::new(env));

    let mut entries = soroban_sdk::Vec::new(env);

    for i in 0..history.len() {
        let result = history.get(i).unwrap();
        entries.push_back(EventIndexEntry {
            position: i,
            outage_id: result.outage_id.clone(),
            recorded_at: result.recorded_at,
            status: result.status.clone(),
            payment_type: result.payment_type.clone(),
        });
    }

    let index = EventIndex {
        count: entries.len(),
        entries,
    };

    env.storage().instance().set(&EVENT_INDEX_KEY, &index);
    Ok(index)
}

/// Get the event index (rebuilds if not present).
pub fn get_event_index(env: &Env) -> Result<EventIndex, SLAError> {
    if let Some(index) = env.storage().instance().get(&EVENT_INDEX_KEY) {
        Ok(index)
    } else {
        build_event_index(env)
    }
}

/// Find events by outage ID using the index.
pub fn find_events_by_outage(
    env: &Env,
    outage_id: &Symbol,
) -> Result<soroban_sdk::Vec<u32>, SLAError> {
    let index = get_event_index(env)?;
    let mut positions = soroban_sdk::Vec::new(env);

    for i in 0..index.entries.len() {
        let entry = index.entries.get(i).unwrap();
        if entry.outage_id == *outage_id {
            positions.push_back(entry.position);
        }
    }

    Ok(positions)
}

/// Find events by status using the index.
pub fn find_events_by_status(
    env: &Env,
    status: &Symbol,
) -> Result<soroban_sdk::Vec<u32>, SLAError> {
    let index = get_event_index(env)?;
    let mut positions = soroban_sdk::Vec::new(env);

    for i in 0..index.entries.len() {
        let entry = index.entries.get(i).unwrap();
        if entry.status == *status {
            positions.push_back(entry.position);
        }
    }

    Ok(positions)
}

/// Find events by payment type using the index.
pub fn find_events_by_payment_type(
    env: &Env,
    payment_type: &Symbol,
) -> Result<soroban_sdk::Vec<u32>, SLAError> {
    let index = get_event_index(env)?;
    let mut positions = soroban_sdk::Vec::new(env);

    for i in 0..index.entries.len() {
        let entry = index.entries.get(i).unwrap();
        if entry.payment_type == *payment_type {
            positions.push_back(entry.position);
        }
    }

    Ok(positions)
}

/// Find events in a time range using the index.
pub fn find_events_in_range(
    env: &Env,
    from_timestamp: u64,
    to_timestamp: u64,
) -> Result<soroban_sdk::Vec<u32>, SLAError> {
    let index = get_event_index(env)?;
    let mut positions = soroban_sdk::Vec::new(env);

    for i in 0..index.entries.len() {
        let entry = index.entries.get(i).unwrap();
        if entry.recorded_at >= from_timestamp && entry.recorded_at <= to_timestamp {
            positions.push_back(entry.position);
        }
    }

    Ok(positions)
}

/// Get the index metadata.
pub fn get_index_metadata(
    env: &Env,
) -> Result<(u32, u64), SLAError> {
    let index = get_event_index(env)?;
    let latest_timestamp = if index.entries.len() > 0 {
        index.entries.get(index.entries.len() - 1).unwrap().recorded_at
    } else {
        0
    };
    Ok((index.count, latest_timestamp))
}

/// Invalidate the index (call after mutations).
pub fn invalidate_index(env: &Env) {
    env.storage().instance().remove(&EVENT_INDEX_KEY);
}
