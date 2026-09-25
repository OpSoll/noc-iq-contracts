// Parameter boundary constraints validation guard. Complements
// config_bundle.rs.
use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum InvalidConfigParameter {
    SlaTargetOutOfRange = 1,
    PenaltyBpsOutOfRange = 2,
}

const MIN_SLA_TARGET_BPS: u32 = 5_000; // 50.0%
const MAX_SLA_TARGET_BPS: u32 = 10_000; // 100.0%
const MAX_PENALTY_BPS: u32 = 10_000; // 100%

/// Validates SLA uptime target (50%-100%) and penalty bps (0-10000)
/// before writing state.
pub fn assert_params_within_bounds(
    sla_target_bps: u32,
    penalty_bps: u32,
) -> Result<(), InvalidConfigParameter> {
    if sla_target_bps < MIN_SLA_TARGET_BPS || sla_target_bps > MAX_SLA_TARGET_BPS {
        return Err(InvalidConfigParameter::SlaTargetOutOfRange);
    }
    if penalty_bps > MAX_PENALTY_BPS {
        return Err(InvalidConfigParameter::PenaltyBpsOutOfRange);
    }
    Ok(())
}
