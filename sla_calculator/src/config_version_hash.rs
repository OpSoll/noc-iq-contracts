// Configuration version hash tracking. Complements config_bundle.rs.
use soroban_sdk::{xdr::ToXdr, Address, Bytes, BytesN, Env, Map, Symbol, Vec};

/// Computes the SHA256 version hash of the active configuration bytes
/// (caller serializes the SLA threshold parameters into `config_bytes`).
pub fn compute_config_version_hash(env: &Env, config_bytes: &Bytes) -> BytesN<32> {
    env.crypto().sha256(config_bytes).into()
}

/// Getter mirroring get_config_version_hash: returns the hash for a
/// given serialized config snapshot.
pub fn get_config_version_hash(env: &Env, config_bytes: &Bytes) -> BytesN<32> {
    compute_config_version_hash(env, config_bytes)
}

pub fn compute_config_map_digest(
    env: &Env,
    configs: &Map<Symbol, u32>,
    schema_version: u32,
) -> BytesN<32> {
    let mut keys = Vec::new(env);
    let mut values = Vec::new(env);
    for (severity, value) in configs.iter() {
        let key = severity.to_val().get_payload();
        let mut at = keys.len();
        while at > 0 && keys.get(at - 1).unwrap() > key {
            at -= 1;
        }
        keys.insert(at, key);
        values.insert(at, value);
    }

    let mut preimage = Bytes::new(env);
    preimage.extend_from_slice(b"config-map-v1");
    preimage.extend_from_slice(&schema_version.to_be_bytes());
    for i in 0..keys.len() {
        preimage.extend_from_slice(&keys.get(i).unwrap().to_be_bytes());
        preimage.extend_from_slice(&values.get(i).unwrap().to_be_bytes());
    }
    compute_config_version_hash(env, &preimage)
}

pub fn compute_config_digest(
    env: &Env,
    configs: &Map<Symbol, u32>,
    schema_version: u32,
) -> BytesN<32> {
    compute_config_map_digest(env, configs, schema_version)
}

pub fn compute_change_digest(
    env: &Env,
    actor: &Address,
    revision: u32,
    severity: &Symbol,
    old_threshold: u32,
    new_threshold: u32,
    old_penalty: i128,
    new_penalty: i128,
    old_reward: i128,
    new_reward: i128,
) -> BytesN<32> {
    let mut preimage = Bytes::new(env);
    preimage.extend_from_slice(b"config-change-v1");
    preimage.extend_from_slice(&revision.to_be_bytes());
    preimage.extend_from_slice(&severity.to_val().get_payload().to_be_bytes());
    preimage.append(&actor.to_xdr(env));
    preimage.extend_from_slice(&old_threshold.to_be_bytes());
    preimage.extend_from_slice(&new_threshold.to_be_bytes());
    preimage.extend_from_slice(&old_penalty.to_be_bytes());
    preimage.extend_from_slice(&new_penalty.to_be_bytes());
    preimage.extend_from_slice(&old_reward.to_be_bytes());
    preimage.extend_from_slice(&new_reward.to_be_bytes());
    compute_config_version_hash(env, &preimage)
}

/// #1021 — outcome of two interleaved revision computations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RevisionOutcome {
    /// Both interleavings produced the same configuration revision.
    Converged(BytesN<32>),
    /// The interleavings disagree; the conflict is surfaced rather than
    /// silently picking a winner.
    Conflict,
}

/// #1021 — deterministically reconcile the revision observed by an operator
/// action with the revision observed by a configuration update.
///
/// The function is symmetric, so both action orders yield the same result:
/// either a single converged revision or a documented conflict.
pub fn reconcile_revisions(operator_revision: &BytesN<32>, config_revision: &BytesN<32>) -> RevisionOutcome {
    if operator_revision == config_revision {
        RevisionOutcome::Converged(operator_revision.clone())
    } else {
        RevisionOutcome::Conflict
    }
}

#[cfg(test)]
mod interleave_tests {
    use super::*;
    use soroban_sdk::Bytes;

    #[test]
    fn both_action_orders_converge_when_revisions_match() {
        let env = Env::default();
        let revision = BytesN::from_array(&env, &[7u8; 32]);
        let operator_then_config = reconcile_revisions(&revision, &revision);
        let config_then_operator = reconcile_revisions(&revision, &revision);
        assert_eq!(operator_then_config, config_then_operator);
        assert_eq!(operator_then_config, RevisionOutcome::Converged(revision));
    }

    #[test]
    fn both_action_orders_report_the_same_conflict() {
        let env = Env::default();
        let operator = BytesN::from_array(&env, &[1u8; 32]);
        let config = BytesN::from_array(&env, &[2u8; 32]);
        assert_eq!(reconcile_revisions(&operator, &config), RevisionOutcome::Conflict);
        assert_eq!(reconcile_revisions(&config, &operator), RevisionOutcome::Conflict);
    }

    #[test]
    fn hashed_configuration_is_a_stable_revision() {
        let env = Env::default();
        let bytes = Bytes::from_slice(&env, b"threshold=30;penalty=50");
        assert_eq!(get_config_version_hash(&env, &bytes), compute_config_version_hash(&env, &bytes));
    }
}

#[cfg(test)]
mod digest_stability_tests {
    use super::*;
    use soroban_sdk::{symbol_short, testutils::Address as _, Address};

    #[test]
    fn change_digest_binds_actor_revision_and_values() {
        let env = Env::default();
        let actor = Address::generate(&env);
        let other = Address::generate(&env);
        let severity = symbol_short!("critical");
        let digest = compute_change_digest(&env, &actor, 3, &severity, 15, 30, 10, 20, 100, 200);

        assert_ne!(
            digest,
            compute_change_digest(&env, &other, 3, &severity, 15, 30, 10, 20, 100, 200)
        );
        assert_ne!(
            digest,
            compute_change_digest(&env, &actor, 4, &severity, 15, 30, 10, 20, 100, 200)
        );
        assert_eq!(
            digest,
            compute_change_digest(&env, &actor, 3, &severity, 15, 30, 10, 20, 100, 200)
        );
    }
}
