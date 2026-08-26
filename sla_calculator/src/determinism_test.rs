#[cfg(test)]
mod determinism_tests {
    use crate::calculator::{calculate_sla, SLACalculationInput};
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(1000))]

        #[test]
        fn prop_sla_calculation_determinism(
            total_requests in 100u64..1_000_000,
            failed_requests in 0u64..50_000,
            response_time_ms in 10u64..5_000,
            downtime_minutes in 0u64..1_440,
        ) {
            // Ensure failed requests do not exceed total requests
            let actual_failures = failed_requests % (total_requests + 1);

            let input = SLACalculationInput {
                total_requests,
                failed_requests: actual_failures,
                response_time_ms,
                downtime_minutes,
            };

            // Execute calculation twice with identical input values
            let result_first = calculate_sla(&input);
            let result_second = calculate_sla(&input);

            // Assert absolute determinism across 1,000 runs
            prop_assert_eq!(result_first, result_second);
        }
    }
}