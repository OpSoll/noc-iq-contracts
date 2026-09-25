use soroban_sdk::Env;

/// Minimum instance-storage TTL (in ledgers) the contract should maintain.
pub const MIN_INSTANCE_TTL_LEDGERS: u32 = 100_000;
/// Amount to bump the TTL by once it drops below the threshold.
pub const INSTANCE_TTL_BUMP_LEDGERS: u32 = 200_000;

/// Ensures instance storage TTL never drops below `MIN_INSTANCE_TTL_LEDGERS`.
/// Intended to be called at the top of state-mutating entry points.
pub fn ensure_instance_ttl(env: &Env) {
    env.storage()
        .instance()
        .extend_ttl(MIN_INSTANCE_TTL_LEDGERS, INSTANCE_TTL_BUMP_LEDGERS);
}
