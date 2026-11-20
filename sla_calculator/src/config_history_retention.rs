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
    let active_revision = active_config_revision(history).unwrap_or(0);
    prune_config_history(history, active_revision);
}

pub fn active_config_revision(history: &Vec<ConfigVersionRecord>) -> Option<u32> {
    let mut active = None;
    for i in 0..history.len() {
        let version = history.get(i).unwrap().version;
        if active.map_or(true, |current| version > current) {
            active = Some(version);
        }
    }
    active
}

pub fn config_revision_is_retained(history: &Vec<ConfigVersionRecord>, revision: u32) -> bool {
    for i in 0..history.len() {
        if history.get(i).unwrap().version == revision {
            return true;
        }
    }
    false
}

pub fn prune_config_history(history: &mut Vec<ConfigVersionRecord>, active_revision: u32) {
    while history.len() > MAX_HISTORY_ENTRIES {
        let mut removable = None;
        for i in 0..history.len() {
            if history.get(i).unwrap().version != active_revision {
                removable = Some(i);
                break;
            }
        }
        match removable {
            Some(index) => {
                history.remove(index);
            }
            None => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(version: u32) -> ConfigVersionRecord {
        ConfigVersionRecord {
            version,
            applied_at: version as u64,
        }
    }

    fn retained_versions(env: &Env, history: &Vec<ConfigVersionRecord>) -> Vec<u32> {
        let mut versions = Vec::new(env);
        for i in 0..history.len() {
            versions.push_back(history.get(i).unwrap().version);
        }
        versions
    }

    #[test]
    fn active_revision_is_queryable_before_at_and_after_cutoff() {
        let env = Env::default();
        for count in [
            MAX_HISTORY_ENTRIES - 1,
            MAX_HISTORY_ENTRIES,
            MAX_HISTORY_ENTRIES + 1,
        ] {
            let mut history = Vec::new(&env);
            for version in 1..=count {
                push_config_history(&env, &mut history, record(version));
            }

            assert!(history.len() <= MAX_HISTORY_ENTRIES);
            assert_eq!(active_config_revision(&history), Some(count));
            assert!(config_revision_is_retained(&history, count));
        }
    }

    #[test]
    fn pruning_keeps_the_active_revision_on_the_oldest_boundary() {
        let env = Env::default();
        let mut history = Vec::new(&env);
        history.push_back(record(MAX_HISTORY_ENTRIES));
        for version in 1..MAX_HISTORY_ENTRIES {
            history.push_back(record(version));
        }
        history.push_back(record(MAX_HISTORY_ENTRIES + 1));

        prune_config_history(&mut history, MAX_HISTORY_ENTRIES);

        assert_eq!(history.len(), MAX_HISTORY_ENTRIES);
        assert!(config_revision_is_retained(&history, MAX_HISTORY_ENTRIES));
        assert!(!config_revision_is_retained(&history, 1));

        let before = retained_versions(&env, &history);
        prune_config_history(&mut history, MAX_HISTORY_ENTRIES);
        assert_eq!(retained_versions(&env, &history), before);
    }

    #[test]
    fn repeated_pruning_after_each_push_is_safe() {
        let env = Env::default();
        let mut history = Vec::new(&env);
        for version in 1..=50 {
            push_config_history(&env, &mut history, record(version));
            let before = history.len();

            prune_config_history(&mut history, version);

            assert_eq!(history.len(), before);
            assert!(config_revision_is_retained(&history, version));
        }
    }
}
