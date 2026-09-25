// Config version negotiation query for client SDKs. Complements
// version_negotiation.rs.
use soroban_sdk::contracttype;

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum CompatibilityStatus {
    Compatible,
    UpgradeRecommended,
    Incompatible,
}

/// Compares the active contract config version against a client's
/// reported version to determine compatibility status.
pub fn check_config_compatibility(
    active_version: u32,
    client_version: u32,
) -> CompatibilityStatus {
    if client_version == active_version {
        CompatibilityStatus::Compatible
    } else if client_version + 1 == active_version {
        CompatibilityStatus::UpgradeRecommended
    } else {
        CompatibilityStatus::Incompatible
    }
}
