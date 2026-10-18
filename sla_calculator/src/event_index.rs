#![no_std]

use soroban_sdk::{symbol_short, Env, Symbol};

use crate::{SLAResult, SLAError, HISTORY_KEY};

// -----------------------------------------------------------------------
// Storage keys
// -----------------------------------------------------------------------
const EVENT_INDEX_KEY: Symbol = symbol_short!("EIDX");
const MAX_INDEX_SIZE_KEY: Symbol = symbol_short!("EIDXMAX");
const DEFAULT_MAX_INDEX_SIZE: u32 = 1000;

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
    let max_size = get_max_index_size(env);

    for i in 0..history.len() {
        if entries.len() >= max_size {
            break;
        }
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

/// Get the configured maximum number of index entries.
pub fn get_max_index_size(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&MAX_INDEX_SIZE_KEY)
        .unwrap_or(DEFAULT_MAX_INDEX_SIZE)
}

/// Configure the maximum number of index entries.
pub fn set_max_index_size(env: &Env, max_size: u32) {
    env.storage().instance().set(&MAX_INDEX_SIZE_KEY, &max_size);
}

/// Remove index entries recorded before `cutoff`; returns how many were pruned.
pub fn prune_index_before(env: &Env, cutoff: u64) -> Result<u32, SLAError> {
    let index = get_event_index(env)?;
    let mut kept = soroban_sdk::Vec::new(env);

    for i in 0..index.entries.len() {
        let entry = index.entries.get(i).unwrap();
        if entry.recorded_at >= cutoff {
            kept.push_back(entry);
        }
    }

    let pruned = index.count - kept.len();
    let updated = EventIndex {
        count: kept.len(),
        entries: kept,
    };
    env.storage().instance().set(&EVENT_INDEX_KEY, &updated);
    Ok(pruned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SLACalculatorContract;

    fn sample_result(env: &Env, id: &str, recorded_at: u64) -> SLAResult {
        SLAResult {
            outage_id: Symbol::new(env, id),
            status: symbol_short!("met"),
            mttr_minutes: 10,
            threshold_minutes: 30,
            amount: 100,
            payment_type: symbol_short!("rew"),
            rating: symbol_short!("good"),
            config_version_hash: 1,
            recorded_at,
        }
    }

    fn sample_entry(env: &Env, position: u32, recorded_at: u64) -> EventIndexEntry {
        EventIndexEntry {
            position,
            outage_id: Symbol::new(env, "OUT"),
            recorded_at,
            status: symbol_short!("met"),
            payment_type: symbol_short!("rew"),
        }
    }

    #[test]
    fn index_size_never_exceeds_the_configured_maximum() {
        let env = Env::default();
        let contract = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&contract, || {
            let mut history = soroban_sdk::Vec::new(&env);
            for i in 0..3u32 {
                history.push_back(sample_result(&env, "OUT", i as u64));
            }
            env.storage().instance().set(&HISTORY_KEY, &history);
            set_max_index_size(&env, 2);
            let index = build_event_index(&env).ok().unwrap();
            assert_eq!(index.count, 2);
            assert_eq!(index.entries.len(), 2);
        });
    }

    #[test]
    fn pruning_removes_only_entries_before_the_cutoff() {
        let env = Env::default();
        let contract = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&contract, || {
            let mut entries = soroban_sdk::Vec::new(&env);
            entries.push_back(sample_entry(&env, 0, 10));
            entries.push_back(sample_entry(&env, 1, 20));
            entries.push_back(sample_entry(&env, 2, 30));
            let index = EventIndex { count: 3, entries };
            env.storage().instance().set(&EVENT_INDEX_KEY, &index);

            assert_eq!(prune_index_before(&env, 20).ok(), Some(1));

            let pruned = get_event_index(&env).ok().unwrap();
            assert_eq!(pruned.count, 2);
            assert_eq!(pruned.entries.get(0).unwrap().position, 1);
            assert_eq!(pruned.entries.get(1).unwrap().position, 2);
        });
    }
}
