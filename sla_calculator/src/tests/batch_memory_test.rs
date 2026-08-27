#[cfg(test)]
mod batch_memory_tests {
    use super::*;
    use soroban_sdk::{vec, Env};

    #[test]
    fn test_optimized_batch_allocation_and_accuracy() {
        let env = Env::default();
        let input_vec = vec![
            &env,
            OptimizedBatchItem { id: 1, score: 45 },
            OptimizedBatchItem { id: 2, score: 80 },
            OptimizedBatchItem { id: 3, score: 95 },
        ];

        let results = OptimizedBatchCalculator::process_optimized_batch(&env, &input_vec);

        assert_eq!(results.len(), 3);
        assert_eq!(results.get(0).unwrap().score, 90);
        assert_eq!(results.get(1).unwrap().score, 160);
        assert_eq!(results.get(2).unwrap().score, 190);
    }
}