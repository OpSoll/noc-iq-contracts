use soroban_sdk::{contracttype, symbol_short, Env, Symbol};

const STATS_KEY: Symbol = symbol_short!("STATS2");

#[contracttype]
#[derive(Clone)]
pub struct SLAStats {
    pub total_calculations: u64,
    pub met_count: u64,
    pub violation_count: u64,
    pub total_penalty: i128,
}

/// Loads the running SLA stats counter, defaulting to all-zero.
pub fn get_stats(env: &Env) -> SLAStats {
    env.storage()
        .instance()
        .get(&STATS_KEY)
        .unwrap_or(SLAStats {
            total_calculations: 0,
            met_count: 0,
            violation_count: 0,
            total_penalty: 0,
        })
}

/// Atomically records the outcome of one SLA calculation.
pub fn record_calculation(env: &Env, met: bool, penalty: i128) {
    let mut stats = get_stats(env);
    stats.total_calculations += 1;
    if met {
        stats.met_count += 1;
    } else {
        stats.violation_count += 1;
        stats.total_penalty += penalty;
    }
    env.storage().instance().set(&STATS_KEY, &stats);
}
