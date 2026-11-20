use crate::SLAResult;
use soroban_sdk::{contracterror, contracttype, symbol_short, Env, Symbol, Vec};

// -----------------------------------------------------------------------
// Storage keys & events
// -----------------------------------------------------------------------
const MONTHLY_SNAPSHOT_PREFIX: Symbol = symbol_short!("MSNAP");
const SNAPSHOT_COUNT_KEY: Symbol = symbol_short!("MSNAPCT");
const EVENT_MONTHLY_SNAP: Symbol = symbol_short!("m_snap");
const EVENT_VERSION: Symbol = symbol_short!("v1");

// -----------------------------------------------------------------------
// Errors
// -----------------------------------------------------------------------
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum SnapshotError {
    AlreadyFinalized = 1,
    InvalidMonthIndex = 2,
    SnapshotNotFound = 3,
}

// -----------------------------------------------------------------------
// Types
// -----------------------------------------------------------------------

/// Normalized view of historical calculations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedSnapshot {
    pub count: u32,
    pub has_violations: bool,
    pub has_rewards: bool,
}

impl NormalizedSnapshot {
    pub fn empty() -> Self {
        Self {
            count: 0,
            has_violations: false,
            has_rewards: false,
        }
    }
}

/// Monthly SLA compliance summary record stored in persistent contract storage.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MonthlySlaSnapshot {
    /// Month identifier (e.g. 202609 or index representing the month).
    pub month_index: u32,
    /// Total aggregate downtime in minutes for the month.
    pub total_downtime_minutes: u32,
    /// Mean time to recovery (MTTR) in minutes.
    pub mttr_minutes: u32,
    /// Final compliance status (e.g. "met" or "viol").
    pub final_compliance_status: Symbol,
    /// Ledger timestamp when snapshot was permanently recorded.
    pub recorded_at: u64,
}

// -----------------------------------------------------------------------
// Functions
// -----------------------------------------------------------------------

pub fn normalize_history(history: &Vec<SLAResult>) -> NormalizedSnapshot {
    if history.len() == 0 {
        return NormalizedSnapshot::empty();
    }

    let mut has_violations = false;
    let mut has_rewards = false;

    for i in 0..history.len() {
        let entry = history.get(i).unwrap();
        if entry.status == Symbol::new(history.env(), "viol") {
            has_violations = true;
        }
        if entry.payment_type == Symbol::new(history.env(), "rew") {
            has_rewards = true;
        }
    }

    NormalizedSnapshot {
        count: history.len(),
        has_violations,
        has_rewards,
    }
}

pub fn normalize_history_window(
    env: &Env,
    from_timestamp: u64,
    to_timestamp: u64,
) -> NormalizedSnapshot {
    if to_timestamp < from_timestamp {
        return NormalizedSnapshot::empty();
    }
    let history: Vec<SLAResult> = env
        .storage()
        .instance()
        .get(&crate::HISTORY_KEY)
        .unwrap_or_else(|| Vec::new(env));
    let mut window = Vec::new(env);
    for entry in history.iter() {
        if entry.recorded_at >= from_timestamp && entry.recorded_at <= to_timestamp {
            window.push_back(entry);
        }
    }
    normalize_history(&window)
}

/// Executes at end of month to write immutable monthly SLA compliance summary records.
///
/// Acceptance criteria:
/// - Saves month identifier, total downtime, MTTR, and final compliance status
/// - Stores monthly records in persistent contract storage keyed by month index
/// - Prevents overwriting previously finalized monthly snapshots
pub fn snapshot_monthly_sla(
    env: &Env,
    month_index: u32,
    total_downtime_minutes: u32,
    mttr_minutes: u32,
    final_compliance_status: Symbol,
) -> Result<MonthlySlaSnapshot, SnapshotError> {
    if month_index == 0 {
        return Err(SnapshotError::InvalidMonthIndex);
    }

    let key = (MONTHLY_SNAPSHOT_PREFIX, month_index);

    // Prevent overwriting previously finalized monthly snapshots
    if env.storage().persistent().has(&key) {
        return Err(SnapshotError::AlreadyFinalized);
    }

    let snapshot = MonthlySlaSnapshot {
        month_index,
        total_downtime_minutes,
        mttr_minutes,
        final_compliance_status: final_compliance_status.clone(),
        recorded_at: env.ledger().timestamp(),
    };

    env.storage().persistent().set(&key, &snapshot);

    let finalized: u32 = env
        .storage()
        .persistent()
        .get(&SNAPSHOT_COUNT_KEY)
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&SNAPSHOT_COUNT_KEY, &(finalized + 1));

    env.events().publish(
        (EVENT_MONTHLY_SNAP, EVENT_VERSION),
        (month_index, final_compliance_status),
    );

    Ok(snapshot)
}

/// Retrieves a finalized monthly SLA summary snapshot from persistent storage by month index.
/// Returns the number of finalized monthly snapshots, for pagination.
pub fn snapshot_count(env: &Env) -> u32 {
    env.storage()
        .persistent()
        .get(&SNAPSHOT_COUNT_KEY)
        .unwrap_or(0)
}

pub fn get_monthly_sla_snapshot(
    env: &Env,
    month_index: u32,
) -> Result<MonthlySlaSnapshot, SnapshotError> {
    let key = (MONTHLY_SNAPSHOT_PREFIX, month_index);
    env.storage()
        .persistent()
        .get(&key)
        .ok_or(SnapshotError::SnapshotNotFound)
}

/// Checks whether a snapshot for the specified month index already exists in persistent storage.
pub fn has_monthly_sla_snapshot(env: &Env, month_index: u32) -> bool {
    let key = (MONTHLY_SNAPSHOT_PREFIX, month_index);
    env.storage().persistent().has(&key)
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SLACalculatorContract, SLACalculatorContractClient, SLAResult, HISTORY_KEY};
    use soroban_sdk::{
        symbol_short,
        testutils::{Address as _, Ledger as _},
        Address, BytesN, Env, Symbol, Vec,
    };

    #[test]
    fn test_snapshot_monthly_sla_storage_layout() {
        let env = Env::default();
        let cid = env.register_contract(None, SLACalculatorContract);

        env.ledger().set_timestamp(1_700_000_000);

        env.as_contract(&cid, || {
            let month_index = 202609;
            let total_downtime = 180;
            let mttr = 45;
            let status = symbol_short!("met");

            assert!(!has_monthly_sla_snapshot(&env, month_index));

            let snapshot =
                snapshot_monthly_sla(&env, month_index, total_downtime, mttr, status.clone())
                    .unwrap();

            assert_eq!(snapshot.month_index, month_index);
            assert_eq!(snapshot.total_downtime_minutes, total_downtime);
            assert_eq!(snapshot.mttr_minutes, mttr);
            assert_eq!(snapshot.final_compliance_status, status);
            assert_eq!(snapshot.recorded_at, 1_700_000_000);

            // Verify retrieval via persistent storage key
            let key = (MONTHLY_SNAPSHOT_PREFIX, month_index);
            let raw_stored: MonthlySlaSnapshot = env.storage().persistent().get(&key).unwrap();
            assert_eq!(raw_stored, snapshot);

            // Verify retrieval via helper function
            let fetched = get_monthly_sla_snapshot(&env, month_index).unwrap();
            assert_eq!(fetched, snapshot);
            assert!(has_monthly_sla_snapshot(&env, month_index));
        });
    }

    #[test]
    fn test_prevents_overwriting_finalized_snapshot() {
        let env = Env::default();
        let cid = env.register_contract(None, SLACalculatorContract);

        env.as_contract(&cid, || {
            let month_index = 202609;

            let first_snap =
                snapshot_monthly_sla(&env, month_index, 100, 25, symbol_short!("met")).unwrap();

            // Attempting to overwrite the finalized snapshot must fail with AlreadyFinalized
            let result = snapshot_monthly_sla(&env, month_index, 500, 120, symbol_short!("viol"));
            assert_eq!(result, Err(SnapshotError::AlreadyFinalized));

            // Verify the original snapshot was not modified
            let stored = get_monthly_sla_snapshot(&env, month_index).unwrap();
            assert_eq!(
                stored.total_downtime_minutes,
                first_snap.total_downtime_minutes
            );
            assert_eq!(
                stored.final_compliance_status,
                first_snap.final_compliance_status
            );
        });
    }

    #[test]
    fn test_invalid_month_index() {
        let env = Env::default();
        let cid = env.register_contract(None, SLACalculatorContract);

        env.as_contract(&cid, || {
            let result = snapshot_monthly_sla(&env, 0, 100, 25, symbol_short!("met"));
            assert_eq!(result, Err(SnapshotError::InvalidMonthIndex));
        });
    }

    #[test]
    fn test_history_snapshot_is_deterministic() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, SLACalculatorContract);
        let client = SLACalculatorContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let operator = Address::generate(&env);
        client.initialize(&admin, &operator);
        client.calculate_sla(
            &operator,
            &symbol_short!("OUT1"),
            &symbol_short!("high"),
            &10,
        );
        client.calculate_sla(
            &operator,
            &symbol_short!("OUT2"),
            &symbol_short!("high"),
            &10,
        );
        let stats = client.get_stats();
        assert_eq!(stats.total_calculations, 2);
    }

    #[test]
    fn snapshot_count_tracks_finalized_snapshots() {
        let env = Env::default();
        let cid = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&cid, || {
            assert_eq!(snapshot_count(&env), 0);
            snapshot_monthly_sla(&env, 202609, 100, 25, symbol_short!("met")).unwrap();
            snapshot_monthly_sla(&env, 202610, 120, 30, symbol_short!("viol")).unwrap();
            assert_eq!(snapshot_count(&env), 2);
        });
    }

    fn result(env: &Env, status: Symbol, payment: Symbol, at: u64) -> SLAResult {
        SLAResult {
            outage_id: symbol_short!("o"),
            status,
            mttr_minutes: 10,
            threshold_minutes: 30,
            amount: 100,
            payment_type: payment,
            rating: symbol_short!("good"),
            config_version_hash: BytesN::from_array(env, &[0u8; 32]),
            recorded_at: at,
        }
    }

    #[test]
    fn empty_and_future_only_windows_normalize_to_the_documented_empty_result() {
        let env = Env::default();
        let cid = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&cid, || {
            assert_eq!(
                normalize_history(&Vec::new(&env)),
                NormalizedSnapshot::empty()
            );
            assert_eq!(
                normalize_history_window(&env, 200, 300),
                NormalizedSnapshot::empty()
            );
            assert_eq!(
                normalize_history_window(&env, 300, 200),
                NormalizedSnapshot::empty()
            );
        });
    }

    #[test]
    fn window_normalization_only_counts_in_range_entries() {
        let env = Env::default();
        let cid = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&cid, || {
            let mut history: Vec<SLAResult> = Vec::new(&env);
            history.push_back(result(
                &env,
                symbol_short!("met"),
                symbol_short!("rew"),
                100,
            ));
            history.push_back(result(
                &env,
                symbol_short!("viol"),
                symbol_short!("pen"),
                200,
            ));
            history.push_back(result(
                &env,
                symbol_short!("met"),
                symbol_short!("rew"),
                300,
            ));
            env.storage().instance().set(&HISTORY_KEY, &history);

            let window = normalize_history_window(&env, 100, 200);
            assert_eq!(window.count, 2);
            assert!(window.has_violations);
            assert!(window.has_rewards);
            assert_eq!(
                normalize_history_window(&env, 400, 500),
                NormalizedSnapshot::empty()
            );
        });
    }
}
