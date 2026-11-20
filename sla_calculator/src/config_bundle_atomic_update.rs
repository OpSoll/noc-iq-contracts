use crate::config::{apply_atomic_writes, StorageWriteError};
use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol};

#[contracttype]
pub struct ConfigBundle {
    pub sla_target_bps: u32,
    pub penalty_bps: u32,
    pub threshold_bps: u32,
    pub grace_period_secs: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundleValues {
    pub sla_target_bps: u64,
    pub penalty_bps: u64,
    pub threshold_bps: u64,
    pub grace_period_secs: u64,
    pub revision: u64,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ConfigBundleError {
    OutOfRange = 1,
    StorageWriteFailed = 2,
}

const BUNDLE_UPDATED_EVENT: Symbol = symbol_short!("cfgbndl");

pub const SLA_TARGET_KEY: Symbol = symbol_short!("slatgtbps");
pub const PENALTY_BPS_KEY: Symbol = symbol_short!("penbps");
pub const THRESHOLD_BPS_KEY: Symbol = symbol_short!("thrbps");
pub const GRACE_SECS_KEY: Symbol = symbol_short!("grace");
pub const REVISION_KEY: Symbol = symbol_short!("crev");

pub fn bundle_writes(bundle: &ConfigBundle, new_version: u32) -> [(Symbol, u64); 5] {
    [
        (SLA_TARGET_KEY, bundle.sla_target_bps as u64),
        (PENALTY_BPS_KEY, bundle.penalty_bps as u64),
        (THRESHOLD_BPS_KEY, bundle.threshold_bps as u64),
        (GRACE_SECS_KEY, bundle.grace_period_secs),
        (REVISION_KEY, new_version as u64),
    ]
}

pub fn read_bundle_values(env: &Env) -> BundleValues {
    BundleValues {
        sla_target_bps: env
            .storage()
            .instance()
            .get(&SLA_TARGET_KEY)
            .unwrap_or(0),
        penalty_bps: env.storage().instance().get(&PENALTY_BPS_KEY).unwrap_or(0),
        threshold_bps: env
            .storage()
            .instance()
            .get(&THRESHOLD_BPS_KEY)
            .unwrap_or(0),
        grace_period_secs: env.storage().instance().get(&GRACE_SECS_KEY).unwrap_or(0),
        revision: env.storage().instance().get(&REVISION_KEY).unwrap_or(0),
    }
}

fn env_writer(env: &Env, key: Symbol, value: u64) -> Result<(), StorageWriteError> {
    env.storage().instance().set(&key, &value);
    Ok(())
}

/// Validates and atomically applies a full config bundle update.
pub fn update_config_bundle(
    env: &Env,
    admin: &Address,
    bundle: &ConfigBundle,
    new_version: u32,
) -> Result<(), ConfigBundleError> {
    update_config_bundle_with_writer(env, admin, bundle, new_version, env_writer)
}

pub fn update_config_bundle_with_writer<F>(
    env: &Env,
    admin: &Address,
    bundle: &ConfigBundle,
    new_version: u32,
    write: F,
) -> Result<(), ConfigBundleError>
where
    F: FnMut(&Env, Symbol, u64) -> Result<(), StorageWriteError>,
{
    admin.require_auth();
    if bundle.sla_target_bps > 10_000
        || bundle.penalty_bps > 10_000
        || bundle.threshold_bps > 10_000
    {
        return Err(ConfigBundleError::OutOfRange);
    }

    apply_atomic_writes(env, &bundle_writes(bundle, new_version), write)
        .map_err(|_| ConfigBundleError::StorageWriteFailed)?;

    env.events()
        .publish((BUNDLE_UPDATED_EVENT,), new_version);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SLACalculatorContract;
    use soroban_sdk::testutils::{Address as _, Events as _};

    fn bundle(target: u32, penalty: u32, threshold: u32, grace: u64) -> ConfigBundle {
        ConfigBundle {
            sla_target_bps: target,
            penalty_bps: penalty,
            threshold_bps: threshold,
            grace_period_secs: grace,
        }
    }

    #[test]
    fn failed_later_write_leaves_old_values_event_and_revision_untouched() {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register_contract(None, SLACalculatorContract);
        let admin = Address::generate(&env);
        let old = bundle(9_000, 800, 700, 60);
        let next = bundle(9_500, 900, 750, 120);

        env.as_contract(&id, || {
            for (key, value) in bundle_writes(&old, 4) {
                env.storage().instance().set(&key, &value);
            }
            let before = read_bundle_values(&env);

            let mut calls = 0usize;
            let result = update_config_bundle_with_writer(
                &env,
                &admin,
                &next,
                5,
                |env, key, value| {
                    if calls == 3 {
                        calls += 1;
                        return Err(StorageWriteError::InjectedFault);
                    }
                    calls += 1;
                    env.storage().instance().set(&key, &value);
                    Ok(())
                },
            );

            assert_eq!(result, Err(ConfigBundleError::StorageWriteFailed));
            assert_eq!(read_bundle_values(&env), before);
            assert_eq!(env.events().all().len(), 0);
        });
    }

    #[test]
    fn successful_update_persists_values_revision_and_event() {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register_contract(None, SLACalculatorContract);
        let admin = Address::generate(&env);
        let next = bundle(9_500, 900, 750, 120);

        env.as_contract(&id, || {
            update_config_bundle(&env, &admin, &next, 7).unwrap();
            let values = read_bundle_values(&env);
            assert_eq!(values.sla_target_bps, 9_500);
            assert_eq!(values.penalty_bps, 900);
            assert_eq!(values.threshold_bps, 750);
            assert_eq!(values.grace_period_secs, 120);
            assert_eq!(values.revision, 7);
            assert_eq!(env.events().all().len(), 1);
        });
    }
}
