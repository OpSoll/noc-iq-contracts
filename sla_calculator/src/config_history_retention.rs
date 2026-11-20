// Maximum config history retention limit: caps history to the last 20
// entries. Complements config_bundle.rs.
use soroban_sdk::{contracttype, symbol_short, Env, Symbol, Vec};

const MAX_HISTORY_ENTRIES: u32 = 20;
const CURRENT_REVISION_KEY: Symbol = symbol_short!("CFGCURREV");

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
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

pub fn current_revision(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&CURRENT_REVISION_KEY)
        .unwrap_or(0)
}

pub fn set_current_revision(env: &Env, revision: u32) {
    env.storage().instance().set(&CURRENT_REVISION_KEY, &revision);
}

pub fn find_version(
    history: &Vec<ConfigVersionRecord>,
    version: u32,
) -> Option<ConfigVersionRecord> {
    for i in 0..history.len() {
        let record = history.get(i).unwrap();
        if record.version == version {
            return Some(record);
        }
    }
    None
}

#[cfg(test)]
mod revision_tests {
    use super::*;
    use crate::SLACalculatorContract;

    fn in_contract<R>(f: impl FnOnce(&Env) -> R) -> R {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&id, || f(&env))
    }

    #[test]
    fn current_revision_starts_at_zero_and_can_be_committed() {
        in_contract(|env| {
            assert_eq!(current_revision(env), 0);
            set_current_revision(env, 4);
            assert_eq!(current_revision(env), 4);
        });
    }

    #[test]
    fn a_historical_version_can_be_looked_up() {
        let env = Env::default();
        let mut history = Vec::new(&env);
        push_config_history(
            &env,
            &mut history,
            ConfigVersionRecord {
                version: 2,
                applied_at: 100,
            },
        );
        push_config_history(
            &env,
            &mut history,
            ConfigVersionRecord {
                version: 3,
                applied_at: 200,
            },
        );

        assert_eq!(
            find_version(&history, 2),
            Some(ConfigVersionRecord {
                version: 2,
                applied_at: 100,
            })
        );
        assert_eq!(find_version(&history, 9), None);
    }
}
