use soroban_sdk::{contracttype, Env, Symbol, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OptimizedBatchItem {
    pub id: u64,
    pub score: u64,
}

pub struct OptimizedBatchCalculator;

impl OptimizedBatchCalculator {
    /// Processes a batch of items with pre-allocated vector capacity to optimize
    /// memory and prevent runtime re-allocations in the Soroban host environment.
    pub fn process_optimized_batch(
        env: &Env,
        items: &Vec<OptimizedBatchItem>,
    ) -> Vec<OptimizedBatchItem> {
        let len = items.len() as u32;
        
        // Pre-allocate vector capacity matching requests.len() to minimize re-allocations
        let mut results: Vec<OptimizedBatchItem> = Vec::with_capacity(env, len);

        for item in items.iter() {
            let processed_score = item.score.saturating_mul(2);
            results.push_back(OptimizedBatchItem {
                id: item.id,
                score: processed_score,
            });
        }

        results
    }

    pub fn process_optimized_batch_with_effects(
        env: &Env,
        items: &Vec<OptimizedBatchItem>,
    ) -> Vec<OptimizedBatchItem> {
        let results = Self::process_optimized_batch(env, items);
        crate::batch::commit_optimized_results(env, &results);
        results
    }
}