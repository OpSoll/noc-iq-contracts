// Maximum config history retention limit: caps history to the last 20
// entries. Complements config_bundle.rs.
use soroban_sdk::{contracttype, Env, Vec};

const MAX_HISTORY_ENTRIES: u32 = 20;

#[contracttype]
pub struct ConfigVersionRecord {
    pub version: u32,
    pub applied_at: u64,
}

/// Appends a new version record, evicting the oldest entry once the
/// rolling buffer exceeds MAX_HISTORY_ENTRIES.
pub fn push_config_history(
    env: &Env,
    history: &mut Vec<ConfigVersionRecord>,
    record: ConfigVersionRecord,
) {
    history.push_back(record);
    while history.len() > MAX_HISTORY_ENTRIES {
        history.remove(0);
    }
}
