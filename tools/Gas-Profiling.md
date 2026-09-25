Gas Profiling: Benchmarking instruction costs for batch outage processing
Repo Avatar
OpSoll/noc-iq-contracts
Problem
Processing large batches of outage events risks exceeding Soroban CPU instruction limits per transaction.

Proposed change
Create benchmark test suite measuring instruction consumption scaling from 1 to 50 outage events per batch.

Acceptance criteria
Benchmark test executes batch processing across 1, 10, 25, 50 event payloads
Logs CPU instruction and memory byte usage per batch size
Establishes maximum safe batch size limit based on network cap
Benchmark test suite added to Cargo test profile
Metadata
Suggested labels: enhancement, smart-contracts
Affected contract or module: sla_calculator/src/benchmarks.rs
Dependencies: None

