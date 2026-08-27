use soroban_sdk::Env;

pub const PROTOCOL_VERSION: u32 = 1;
pub const STORAGE_VERSION: u32 = 1;

/// Returns `(protocol_version, storage_version)` for off-chain integrators.
pub fn get_version(_env: &Env) -> (u32, u32) {
    (PROTOCOL_VERSION, STORAGE_VERSION)
}
