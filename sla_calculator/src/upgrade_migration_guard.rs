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
