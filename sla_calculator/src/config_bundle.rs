use crate::{
    config::ConfigValidationError, config_compatibility_check::ConfigInvariantError, SLAConfig,
    SLAConfigEntry, SLAConfigSnapshot, SLAResultSchema, CONFIG_KEY, RESULT_SCHEMA_VERSION,
};
use soroban_sdk::{contracttype, symbol_short, Env, Map, Symbol, Vec};

pub struct ConfigBundle {
    pub snapshot: SLAConfigSnapshot,
    pub schema: SLAResultSchema,
}

const BUNDLE_REVISION_KEY: Symbol = symbol_short!("BUNDLEREV");

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigBundleRevision {
    pub revision: u32,
    pub snapshot: SLAConfigSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApplyBundleError {
    DuplicateSeverity(Symbol),
    InvalidInvariant(ConfigInvariantError),
}

impl From<ConfigValidationError> for ApplyBundleError {
    fn from(error: ConfigValidationError) -> Self {
        match error {
            ConfigValidationError::DuplicateSeverity(severity) => {
                ApplyBundleError::DuplicateSeverity(severity)
            }
        }
    }
}

impl From<ConfigInvariantError> for ApplyBundleError {
    fn from(error: ConfigInvariantError) -> Self {
        ApplyBundleError::InvalidInvariant(error)
    }
}

pub fn read_config_bundle(env: &Env) -> Option<ConfigBundle> {
    let snapshot: SLAConfigSnapshot = env.storage().instance().get(&CONFIG_KEY)?;
    let schema = build_result_schema(env);
    Some(ConfigBundle { snapshot, schema })
}

pub fn read_bundle_revision(env: &Env) -> Option<ConfigBundleRevision> {
    env.storage().instance().get(&BUNDLE_REVISION_KEY)
}

pub fn apply_config_bundle(
    env: &Env,
    entries: &Vec<SLAConfigEntry>,
) -> Result<ConfigBundleRevision, ApplyBundleError> {
    let mut severities = Vec::<Symbol>::new(env);
    for entry in entries.iter() {
        severities.push_back(entry.severity.clone());
    }
    crate::config::validate_unique_severities(env, &severities)?;
    crate::config_compatibility_check::validate_cross_severity_invariants(entries)?;

    let mut configs = Map::<Symbol, SLAConfig>::new(env);
    for entry in entries.iter() {
        configs.set(entry.severity.clone(), entry.config.clone());
    }

    let revision = read_bundle_revision(env).map_or(0, |stored| stored.revision) + 1;
    let stored = ConfigBundleRevision {
        revision,
        snapshot: SLAConfigSnapshot {
            version: symbol_short!("v1"),
            version_hash: crate::config::config_version_hash(env, &configs),
            entries: entries.clone(),
        },
    };

    env.storage().instance().set(&CONFIG_KEY, &configs);
    env.storage().instance().set(&BUNDLE_REVISION_KEY, &stored);
    Ok(stored)
}

fn build_result_schema(env: &Env) -> SLAResultSchema {
    use soroban_sdk::symbol_short;
    SLAResultSchema {
        version: symbol_short!("v1"),
        schema_version: RESULT_SCHEMA_VERSION,
        status_met: symbol_short!("met"),
        status_violated: symbol_short!("viol"),
        payment_reward: symbol_short!("rew"),
        payment_penalty: symbol_short!("pen"),
        rating_exceptional: symbol_short!("top"),
        rating_excellent: symbol_short!("excel"),
        rating_good: symbol_short!("good"),
        rating_poor: symbol_short!("poor"),
        includes_config_version_hash: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SLACalculatorContract, SLACalculatorContractClient};
    use soroban_sdk::{testutils::Address as _, Address};

    fn setup() -> (Env, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, SLACalculatorContract);
        let client = SLACalculatorContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let operator = Address::generate(&env);
        client.initialize(&admin, &operator);
        (env, contract_id)
    }

    fn entry(severity: Symbol, threshold_minutes: u32, penalty_per_minute: i128) -> SLAConfigEntry {
        SLAConfigEntry {
            severity,
            config: SLAConfig {
                threshold_minutes,
                penalty_per_minute,
                reward_base: 500,
                top_tier_multiplier: 200,
                excel_tier_multiplier: 150,
                good_tier_multiplier: 100,
            },
        }
    }

    fn ordered_entries(env: &Env) -> Vec<SLAConfigEntry> {
        soroban_sdk::vec![
            env,
            entry(symbol_short!("critical"), 15, 100),
            entry(symbol_short!("high"), 30, 50),
            entry(symbol_short!("medium"), 60, 20),
            entry(symbol_short!("low"), 120, 5),
        ]
    }

    #[test]
    fn test_config_bundle_available_after_init() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let bundle = read_config_bundle(&env).unwrap();
            assert_eq!(bundle.snapshot.entries.len(), 4);
        });
    }

    #[test]
    fn duplicate_severity_is_rejected_without_touching_the_prior_snapshot() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let before = env.storage().instance().get(&CONFIG_KEY);
            let mut entries = ordered_entries(&env);
            let duplicate = entries.get(1).unwrap();
            entries.push_back(duplicate);

            assert_eq!(
                apply_config_bundle(&env, &entries),
                Err(ApplyBundleError::DuplicateSeverity(symbol_short!("high")))
            );
            assert_eq!(env.storage().instance().get::<Symbol, Map<Symbol, SLAConfig>>(&CONFIG_KEY), before);
            assert_eq!(read_bundle_revision(&env), None);
        });
    }

    #[test]
    fn invalid_cross_severity_bundle_is_rejected_atomically() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let before = env.storage().instance().get(&CONFIG_KEY);
            let entries = soroban_sdk::vec![
                &env,
                entry(symbol_short!("critical"), 60, 100),
                entry(symbol_short!("high"), 30, 50),
                entry(symbol_short!("medium"), 90, 20),
                entry(symbol_short!("low"), 120, 5),
            ];

            assert_eq!(
                apply_config_bundle(&env, &entries),
                Err(ApplyBundleError::InvalidInvariant(
                    ConfigInvariantError::ThresholdOrder(symbol_short!("high"))
                ))
            );
            assert_eq!(env.storage().instance().get::<Symbol, Map<Symbol, SLAConfig>>(&CONFIG_KEY), before);
        });
    }

    #[test]
    fn valid_bundle_produces_one_snapshot_and_revision() {
        let (env, contract_id) = setup();
        env.as_contract(&contract_id, || {
            let first = apply_config_bundle(&env, &ordered_entries(&env)).unwrap();
            assert_eq!(first.revision, 1);

            let second = apply_config_bundle(&env, &ordered_entries(&env)).unwrap();
            assert_eq!(second.revision, 2);
            assert_eq!(second.snapshot.entries.len(), 4);
            assert_eq!(read_bundle_revision(&env), Some(second));
            let stored: Map<Symbol, SLAConfig> = env.storage().instance().get(&CONFIG_KEY).unwrap();
            assert_eq!(stored.len(), 4);
        });
    }
}
