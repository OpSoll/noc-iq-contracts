// Configuration version hash tracking. Complements config_bundle.rs.
use soroban_sdk::{Bytes, BytesN, Env};

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
