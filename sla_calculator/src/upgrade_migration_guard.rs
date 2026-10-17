// Contract upgrade path WASM hash validator / migration compatibility
// check. Complements upgrade_path.rs.
use soroban_sdk::{contracterror, BytesN, Env};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum UpgradeGuardError {
    IncompatibleLayout = 1,
}

/// Blocks a WASM upgrade unless the target hash is on the
/// pre-verified-compatible allowlist (populated by an off-chain
/// migration dry-run before the upgrade is proposed).
pub fn assert_upgrade_compatible(
    target_wasm_hash: &BytesN<32>,
    verified_compatible_hashes: &[BytesN<32>],
) -> Result<(), UpgradeGuardError> {
    if verified_compatible_hashes.contains(target_wasm_hash) {
        Ok(())
    } else {
        Err(UpgradeGuardError::IncompatibleLayout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(env: &Env, byte: u8) -> BytesN<32> {
        BytesN::from_array(env, &[byte; 32])
    }

    #[test]
    fn test_verified_hash_is_allowed() {
        let env = Env::default();
        let allow = [hash(&env, 1), hash(&env, 2)];
        assert_eq!(assert_upgrade_compatible(&hash(&env, 2), &allow), Ok(()));
    }

    #[test]
    fn test_unverified_hash_is_blocked() {
        let env = Env::default();
        let allow = [hash(&env, 1)];
        assert_eq!(
            assert_upgrade_compatible(&hash(&env, 9), &allow),
            Err(UpgradeGuardError::IncompatibleLayout)
        );
    }

    #[test]
    fn test_empty_allowlist_blocks_every_migration() {
        let env = Env::default();
        assert_eq!(
            assert_upgrade_compatible(&hash(&env, 1), &[]),
            Err(UpgradeGuardError::IncompatibleLayout)
        );
    }

    #[test]
    fn test_hash_differing_in_one_byte_is_blocked() {
        let env = Env::default();
        let mut near = [1u8; 32];
        near[31] = 2;
        let allow = [hash(&env, 1)];
        assert_eq!(
            assert_upgrade_compatible(&BytesN::from_array(&env, &near), &allow),
            Err(UpgradeGuardError::IncompatibleLayout)
        );
    }
}
