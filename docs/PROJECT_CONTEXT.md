# NOC IQ System

## Repositories

- noc-iq-fe → frontend
- noc-iq-be → backend
- noc-iq-contracts → smart contracts

## System Flow

User → FE → BE → Contracts → BE → FE

## Rules

- FE never talks to contracts directly
- BE is the bridge
- Contracts are execution layer only

---

## SC-100: Future Contract Roadmap

This section documents the planned evolution of `noc-iq-contracts` based on
current backend integration needs. It distinguishes what exists today from what
is planned, so contributors do not assume missing crates are already present.

### Current State

Only one contract crate exists in this repository:

| Crate | Status | Description |
|---|---|---|
| `sla_calculator` | **Production-ready** | Calculates SLA penalties and rewards; stores config; emits versioned events |

### Planned Additions

The following crates are planned but **not yet implemented**. Do not import or
reference them until they appear in the repository.

| Crate | Status | Depends on | Description |
|---|---|---|---|
| `payment_escrow` | Planned | `sla_calculator` | Locks and conditionally releases Stellar token payments based on SLA results |
| `settlement` | Planned | `payment_escrow` | Splits shared outage costs between multiple parties |
| `governance` | Planned | — | On-chain admin config changes with time-locked execution |

### Integration Expectations

- The backend (`noc-iq-be`) currently integrates only with `sla_calculator`.
- New crates will be introduced incrementally; each must expose a
  `get_result_schema()` equivalent so the backend can version-pin safely.
- Frontend never calls contracts directly — all invocations go through the backend.

### Contribution Guidelines for New Crates

1. Open a tracking issue before creating the crate directory.
2. Follow the `sla_calculator` layout: `src/lib.rs`, `src/tests.rs`, `Cargo.toml`.
3. Add the new crate to any CI matrix in `.github/workflows/`.
4. Export a result schema function so the backend can detect breaking changes.
