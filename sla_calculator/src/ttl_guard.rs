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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SLACalculatorContract;
    use soroban_sdk::testutils::Ledger as _;

    #[test]
    fn test_bump_stays_within_the_network_maximum() {
        let env = Env::default();
        let max = env.ledger().get().max_entry_ttl;
        assert!(INSTANCE_TTL_BUMP_LEDGERS <= max);
        assert!(MIN_INSTANCE_TTL_LEDGERS <= INSTANCE_TTL_BUMP_LEDGERS);
    }

    #[test]
    fn test_ensure_instance_ttl_does_not_exceed_extension_limits() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        // extend_ttl traps if asked to extend past the network maximum, so a
        // clean return proves the configured bump is within limits.
        env.as_contract(&id, || ensure_instance_ttl(&env));
    }

    #[test]
    fn test_ensure_instance_ttl_is_safe_to_call_repeatedly() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&id, || {
            ensure_instance_ttl(&env);
            ensure_instance_ttl(&env);
            ensure_instance_ttl(&env);
        });
    }

    #[test]
    fn test_ensure_instance_ttl_after_ledgers_advance() {
        let env = Env::default();
        let id = env.register_contract(None, SLACalculatorContract);
        env.as_contract(&id, || ensure_instance_ttl(&env));
        env.ledger().with_mut(|l| l.sequence_number += 50_000);
        env.as_contract(&id, || ensure_instance_ttl(&env));
    }
}
