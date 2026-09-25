Tooling: Implement deterministic simulation test harness for contract calls
Repo Avatar
OpSoll/noc-iq-contracts
Problem
Integration tests running against live testnets suffer from network latency and non-deterministic state variations.

Proposed change
Create a deterministic local simulation test harness running contract calls in isolated Soroban environment.

Acceptance criteria
Test harness initializes fresh Soroban test environment for each test case
Provides pre-funded mock accounts and deterministic ledger sequences
Allows stepping through contract state changes line-by-line
Unit test verifies simulation harness determinism
Metadata
Suggested labels: enhancement, smart-contracts
Affected contract or module: tests/deterministicSimulationHarness.test.ts
Dependencies: None

