# NOC IQ Smart Contracts

Soroban contract repository for the NOC IQ system.

This repository is the execution-layer side of the 3-repo architecture:

- `noc-iq-fe` -> frontend
- `noc-iq-be` -> backend and integration layer
- `noc-iq-contracts` -> Soroban smart contracts

System flow:

`User -> FE -> BE -> Contracts -> BE -> FE`

Important rule:

- contracts are not called directly by the frontend
- the backend is responsible for invoking contracts and translating results back to the UI

## Overview

`noc-iq-contracts` contains the Soroban-side SLA logic for NOC IQ.

At the current checked-in state, this repository contains one active contract crate:

- `sla_calculator`

This contract is responsible for deterministic SLA calculation and related contract-side state such as configuration, statistics, pause state, and calculation history.

## Current Stack

- Rust
- Soroban SDK 21
- Cargo

Main crate manifest:

- `sla_calculator/Cargo.toml`

## Current Contract Surface

The active contract is in:

- `sla_calculator/src/lib.rs`

The current implementation includes:

- initialization with admin and operator roles
- severity-based SLA configuration
- admin-controlled config updates
- operator-gated `calculate_sla`
- read-only `calculate_sla_view`
- backend-friendly `get_config_snapshot`
- pause and unpause controls
- cumulative SLA statistics
- history retrieval and pruning

Tests live in:

- `sla_calculator/src/tests.rs`

## Project Structure

```text
noc-iq-contracts/
├── docs/
│   ├── CODEX_CONTEXT.md
│   └── PROJECT_CONTEXT.md
├── sla_calculator/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       └── tests.rs
├── CONTRIBUTING.md
├── README.md
```

## What Is Actually In This Repo

Only the SLA calculator contract is currently checked in.

That means this repo does not currently contain:

- `payment_escrow`
- `multi_party_settlement`
- deployment scripts
- a top-level Cargo workspace

If those are planned, they are future work rather than part of the present repository state.

## Local Setup

### Prerequisites

- Rust toolchain
- Cargo
- optional: Soroban CLI for deployment workflows

### Run Tests

```bash
cd sla_calculator
cargo test
```

### Build The Contract

```bash
cd sla_calculator
cargo build
```

### Build WASM

```bash
cd sla_calculator
cargo build --target wasm32-unknown-unknown --release
```

## Verification Notes

As of the latest stabilization pass:

- `cargo test` passes
- the crate compiles cleanly
- the checked-in test suite is wired into the crate and runs

The current test suite covers:

- role and authorization behavior
- SLA reward and penalty logic
- pause and unpause behavior
- statistics
- audit-mode calculation parity
- history recording and pruning

## Backend Relationship

The backend repo is expected to invoke this contract and translate contract results into backend API responses.

That dependency matters because:

- SLA logic must stay aligned with backend expectations
- result encoding must remain deterministic
- API consumers in `noc-iq-fe` only see what `noc-iq-be` returns
- config reads should prefer explicit snapshot-style contract views where stable ordering matters

## Current Limitations

This repository is now stable at the crate level, but the overall contract layer is still narrow.

Examples:

- only one contract crate exists right now
- deployment automation is not checked in
- there is not yet a broader contract workspace with escrow or settlement modules
- cross-repo contract invocation is still a backend integration concern, not something managed here directly

## Related Repositories

- `noc-iq-fe` -> frontend application
- `noc-iq-be` -> backend and contract bridge
