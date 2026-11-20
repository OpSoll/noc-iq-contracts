// Config version negotiation query for client SDKs. Complements
// version_negotiation.rs.
use crate::{SLAConfig, SLAConfigEntry};
use soroban_sdk::{contracttype, symbol_short, Symbol, Vec};

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum CompatibilityStatus {
    Compatible,
    UpgradeRecommended,
    Incompatible,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConfigInvariantError {
    InvalidSeverity(Symbol),
    DuplicateSeverity(Symbol),
    MissingSeverity(Symbol),
    ThresholdOrder(Symbol),
    PenaltyOrder(Symbol),
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

fn severity_index(severity: &Symbol) -> Option<usize> {
    if *severity == symbol_short!("critical") {
        Some(0)
    } else if *severity == symbol_short!("high") {
        Some(1)
    } else if *severity == symbol_short!("medium") {
        Some(2)
    } else if *severity == symbol_short!("low") {
        Some(3)
    } else {
        None
    }
}

pub fn validate_cross_severity_invariants(
    entries: &Vec<SLAConfigEntry>,
) -> Result<(), ConfigInvariantError> {
    let canonical = [
        symbol_short!("critical"),
        symbol_short!("high"),
        symbol_short!("medium"),
        symbol_short!("low"),
    ];
    let mut ordered: [Option<&SLAConfig>; 4] = [None, None, None, None];

    for entry in entries.iter() {
        let index = severity_index(&entry.severity)
            .ok_or_else(|| ConfigInvariantError::InvalidSeverity(entry.severity.clone()))?;
        if ordered[index].is_some() {
            return Err(ConfigInvariantError::DuplicateSeverity(
                entry.severity.clone(),
            ));
        }
        ordered[index] = Some(&entry.config);
    }

    for (index, severity) in canonical.iter().enumerate() {
        if ordered[index].is_none() {
            return Err(ConfigInvariantError::MissingSeverity(severity.clone()));
        }
    }

    for index in 1..canonical.len() {
        let higher = ordered[index - 1].unwrap();
        let lower = ordered[index].unwrap();
        if higher.threshold_minutes >= lower.threshold_minutes {
            return Err(ConfigInvariantError::ThresholdOrder(
                canonical[index].clone(),
            ));
        }
        if higher.penalty_per_minute <= lower.penalty_per_minute {
            return Err(ConfigInvariantError::PenaltyOrder(canonical[index].clone()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

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

    #[test]
    fn ordered_bundle_satisfies_cross_severity_invariants() {
        let env = Env::default();
        let entries = soroban_sdk::vec![
            &env,
            entry(symbol_short!("critical"), 15, 100),
            entry(symbol_short!("high"), 30, 50),
            entry(symbol_short!("medium"), 60, 20),
            entry(symbol_short!("low"), 120, 5),
        ];
        assert_eq!(validate_cross_severity_invariants(&entries), Ok(()));
    }

    #[test]
    fn inverted_threshold_is_rejected_before_application() {
        let env = Env::default();
        let entries = soroban_sdk::vec![
            &env,
            entry(symbol_short!("critical"), 60, 100),
            entry(symbol_short!("high"), 30, 50),
            entry(symbol_short!("medium"), 90, 20),
            entry(symbol_short!("low"), 120, 5),
        ];
        assert_eq!(
            validate_cross_severity_invariants(&entries),
            Err(ConfigInvariantError::ThresholdOrder(symbol_short!("high")))
        );
    }

    #[test]
    fn inverted_penalty_is_rejected_before_application() {
        let env = Env::default();
        let entries = soroban_sdk::vec![
            &env,
            entry(symbol_short!("critical"), 15, 10),
            entry(symbol_short!("high"), 30, 50),
            entry(symbol_short!("medium"), 60, 20),
            entry(symbol_short!("low"), 120, 5),
        ];
        assert_eq!(
            validate_cross_severity_invariants(&entries),
            Err(ConfigInvariantError::PenaltyOrder(symbol_short!("high")))
        );
    }
}
