use soroban_sdk::contracttype;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PenaltyResult {
    pub final_penalty: u64,
    pub is_capped: bool,
}

pub struct OvertimeCalculator;

impl OvertimeCalculator {
    const MAX_MULTIPLIER: u64 = 10;

    /// Calculates overtime penalties, capping the multiplier at 10x base penalty
    /// and flagging metadata if an extreme outage triggers the cap.
    pub fn calculate_overtime_penalty(base_penalty: u64, overtime_multiplier: u64) -> PenaltyResult {
        let mut is_capped = false;
        let mut effective_multiplier = overtime_multiplier;

        if effective_multiplier > Self::MAX_MULTIPLIER {
            effective_multiplier = Self::MAX_MULTIPLIER;
            is_capped = true;
        }

        PenaltyResult {
            final_penalty: base_penalty * effective_multiplier,
            is_capped,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_overtime_calculation() {
        let result = OvertimeCalculator::calculate_overtime_penalty(50, 4);
        assert_eq!(result.final_penalty, 200);
        assert!(!result.is_capped);
    }

    #[test]
    fn test_extreme_overtime_outage_capping() {
        // Multiplier 25 exceeds 10x limit, should cap at 10x and set is_capped to true
        let result = OvertimeCalculator::calculate_overtime_penalty(50, 25);
        assert_eq!(result.final_penalty, 500); // 50 * 10
        assert!(result.is_capped);
    }

    #[test]
    fn test_exact_boundary_multiplier() {
        let result = OvertimeCalculator::calculate_overtime_penalty(100, 10);
        assert_eq!(result.final_penalty, 1000);
        assert!(!result.is_capped);
    }
}