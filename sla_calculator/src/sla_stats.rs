use soroban_sdk::{contracttype, symbol_short, Env, Symbol};

const STATS_KEY: Symbol = symbol_short!("STATS2");

#[contracttype]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SLAStats {
    pub total_calculations: u64,
    pub met_count: u64,
    pub violation_count: u64,
    pub total_penalty: i128,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatsError {
    TotalCalculationsOverflow,
    MetCountOverflow,
    ViolationCountOverflow,
    TotalPenaltyOverflow,
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
pub fn record_calculation(env: &Env, met: bool, penalty: i128) -> Result<(), StatsError> {
    let mut stats = get_stats(env);
    let total_calculations = stats
        .total_calculations
        .checked_add(1)
        .ok_or(StatsError::TotalCalculationsOverflow)?;
    let (met_count, violation_count, total_penalty) = if met {
        (
            stats
                .met_count
                .checked_add(1)
                .ok_or(StatsError::MetCountOverflow)?,
            stats.violation_count,
            stats.total_penalty,
        )
    } else {
        (
            stats.met_count,
            stats
                .violation_count
                .checked_add(1)
                .ok_or(StatsError::ViolationCountOverflow)?,
            stats
                .total_penalty
                .checked_add(penalty)
                .ok_or(StatsError::TotalPenaltyOverflow)?,
        )
    };
    stats.total_calculations = total_calculations;
    stats.met_count = met_count;
    stats.violation_count = violation_count;
    stats.total_penalty = total_penalty;
    env.storage().instance().set(&STATS_KEY, &stats);
    Ok(())
}

pub fn try_record_calculation(env: &Env, met: bool, penalty: i128) -> Result<(), StatsError> {
    record_calculation(env, met, penalty)
}

/// Aggregate `(success, failure)` counts: SLAs met and SLAs violated.
pub fn get_success_failure_counts(env: &Env) -> (u64, u64) {
    let stats = get_stats(env);
    (stats.met_count, stats.violation_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SLACalculatorContract;

    #[test]
    fn test_counts_start_at_zero() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&id, || assert_eq!(get_success_failure_counts(&env), (0, 0)));
    }

    #[test]
    fn test_counts_track_successes_and_failures_separately() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&id, || {
            record_calculation(&env, true, 0).unwrap();
            record_calculation(&env, true, 0).unwrap();
            record_calculation(&env, false, 250).unwrap();
            assert_eq!(get_success_failure_counts(&env), (2, 1));
        });
    }

    #[test]
    fn test_counts_sum_to_total_calculations() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&id, || {
            for i in 0..10u32 {
                record_calculation(&env, i % 3 != 0, 10).unwrap();
            }
            let (ok, bad) = get_success_failure_counts(&env);
            assert_eq!(ok + bad, get_stats(&env).total_calculations);
            assert_eq!((ok, bad), (6, 4));
        });
    }
}

#[cfg(test)]
mod overflow_tests {
    use super::*;
    use crate::SLACalculatorContract;

    fn seed(env: &Env, stats: &SLAStats) {
        env.storage().instance().set(&STATS_KEY, stats);
    }

    fn assert_unchanged(env: &Env, before: &SLAStats) {
        assert_eq!(get_stats(env), *before);
    }

    #[test]
    fn a_total_calculations_overflow_rejects_the_whole_update() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&id, || {
            let before = SLAStats {
                total_calculations: u64::MAX,
                met_count: 7,
                violation_count: 3,
                total_penalty: 25,
            };
            seed(&env, &before);
            assert_eq!(
                record_calculation(&env, true, 0),
                Err(StatsError::TotalCalculationsOverflow)
            );
            assert_unchanged(&env, &before);
        });
    }

    #[test]
    fn a_met_count_overflow_rejects_the_whole_update() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&id, || {
            let before = SLAStats {
                total_calculations: 10,
                met_count: u64::MAX,
                violation_count: 3,
                total_penalty: 25,
            };
            seed(&env, &before);
            assert_eq!(
                try_record_calculation(&env, true, 0),
                Err(StatsError::MetCountOverflow)
            );
            assert_unchanged(&env, &before);
        });
    }

    #[test]
    fn a_violation_count_overflow_rejects_the_whole_update() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&id, || {
            let before = SLAStats {
                total_calculations: 10,
                met_count: 7,
                violation_count: u64::MAX,
                total_penalty: 25,
            };
            seed(&env, &before);
            assert_eq!(
                record_calculation(&env, false, 5),
                Err(StatsError::ViolationCountOverflow)
            );
            assert_unchanged(&env, &before);
        });
    }

    #[test]
    fn a_penalty_total_overflow_rejects_the_whole_update() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&id, || {
            let before = SLAStats {
                total_calculations: 10,
                met_count: 7,
                violation_count: 3,
                total_penalty: i128::MAX,
            };
            seed(&env, &before);
            assert_eq!(
                record_calculation(&env, false, 1),
                Err(StatsError::TotalPenaltyOverflow)
            );
            assert_unchanged(&env, &before);
            assert_eq!(
                record_calculation(&env, false, i128::MIN),
                Err(StatsError::TotalPenaltyOverflow)
            );
            assert_unchanged(&env, &before);
        });
    }

    #[test]
    fn a_boundary_value_that_fits_is_committed() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&id, || {
            seed(
                &env,
                &SLAStats {
                    total_calculations: u64::MAX - 1,
                    met_count: u64::MAX - 1,
                    violation_count: 3,
                    total_penalty: i128::MAX - 5,
                },
            );
            assert_eq!(record_calculation(&env, true, 0), Ok(()));
            let stats = get_stats(&env);
            assert_eq!(stats.total_calculations, u64::MAX);
            assert_eq!(stats.met_count, u64::MAX);
            assert_eq!(stats.violation_count, 3);
            assert_eq!(stats.total_penalty, i128::MAX - 5);
        });
    }
}
