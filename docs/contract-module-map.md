# Contract Module Map and Ownership Hints

**SC-W5-116** | Track: Contracts | Difficulty: Medium

A quick-reference map of every module in `noc-iq-contracts` for new
contributors. Each entry notes the primary responsibility, the main files,
and the issue IDs that created or extend it.

---

## Repository Layout

```
noc-iq-contracts/
├── sla_calculator/src/      # Soroban contract (Rust)
├── offchain/                # Off-chain TypeScript helpers for the backend
├── tooling/                 # DX / release / governance tooling
├── scripts/                 # CI scripts and one-shot runners
├── tools/                   # Miscellaneous developer tools
├── tests/                   # TypeScript integration test fixtures
├── docs/                    # Documentation
├── artifacts/               # Canonical test vector snapshots
└── ts/                      # Shared TypeScript utilities
```

---

## Module Ownership

### `sla_calculator/src/lib.rs`
**Owner:** contracts-core  
**Responsibility:** All on-chain logic — initialization, SLA calculation,
config management, pause/unpause, governance (admin/operator two-step),
stats, and history.  
**Do not** add off-chain logic here. Keep it `#![no_std]`.  
**Key issues:** SC-W5-001 through SC-W5-080

---

### `sla_calculator/src/tests.rs`
**Owner:** contracts-core  
**Responsibility:** Soroban unit and integration tests for the contract.
Add a test for every new function or edge case.  
**Run with:** `cd sla_calculator && cargo test`

---

### `sla_calculator/src/dao_admin_takeover.rs` and `multi_operator.rs`
**Owner:** contracts-core  
**Responsibility:** Emergency DAO admin takeover (propose / approve / execute)
voted on by the multi-operator set. `multi_operator.rs` owns operator
membership and bumps a membership epoch on every add/remove; execution
recomputes the eligible voter set and quorum (strict majority) from live
membership and ignores approvals from addresses that left the set (#1013).
Internal modules only: no `#[contractimpl]` entrypoints yet.

---

### `offchain/`
**Owner:** backend-integration  
**Responsibility:** TypeScript helpers that the backend calls to validate
SLA results, manage tx lifecycle, and reconcile ambiguous outcomes.  
Key files and their SC issues:

| File | Issue | Purpose |
|------|-------|---------|
| `slaPaymentIntentConformance.ts` | SC-W5-110 | SLA->payment mapping validation |
| `txSubmissionAmbiguity.ts`       | SC-W5-111 | Safe retry semantics |
| `txHashCorrelation.ts`           | SC-W5-112 | Duplicate tx prevention |
| `payoutFinalityState.ts`         | SC-W5-113 | Payout state machine |
| `ambiguousOutcomeReplay.ts`      | SC-W5-114 | Reconciliation workflow |
| `failureRecoverySemantics.ts`    | SC-W5-115 | Partial observation recovery |
| `governanceConsistency.ts`       | SC-W5-029 | Event/state consistency |
| `eventSizeRegression.ts`         | SC-W5-029 | Event size budgets |
| `readCostRegression.ts`          | SC-W5-029 | Read cost budgets |

---

### `tooling/`
**Owner:** dx-and-release  
**Responsibility:** DX automation, release gates, and governance summaries.

| File | Issue | Purpose |
|------|-------|---------|
| `releaseChecklist.ts`         | SC-W5-old  | Pre-tag release checks |
| `governanceSummary.ts`        | SC-W5-old  | Governance state aggregation |
| `releaseCandidateGate.ts`     | SC-W5-121  | RC promotion gate |
| `prChecklist.ts`              | SC-W5-119  | PR invariant checklist |
| `roadmapTraceability.ts`      | SC-W5-118  | SC-ID to roadmap validation |
| `rollbackDecisionMatrix.ts`   | SC-W5-124  | HOLD/MONITOR/ROLLBACK logic |
| `waveReliabilityScorecard.ts` | SC-W5-125  | 0-100 release scorecard |

---

### `scripts/`
**Owner:** ci-and-ops  
**Responsibility:** CI-runnable scripts for size checks, tests, security
scans, and deployment verification.

| File | Issue | Purpose |
|------|-------|---------|
| `check-wasm-size.ts`    | SC-W5-old  | WASM size budget enforcement |
| `run-tests.ts`          | SC-W5-old  | Cargo test preset runner |
| `security-gate.ts`      | SC-W5-old  | Staged Rust security scan |
| `pre-deploy-verify.ts`  | SC-W5-122  | Pre-deploy manifest check |
| `post-deploy-smoke.ts`  | SC-W5-123  | Post-deploy entrypoint smoke |

---

### `docs/`
**Owner:** documentation  
**Responsibility:** Context documents for contributors and AI tooling.

| File | Purpose |
|------|---------|
| `CODEX_CONTEXT.md`               | AI/LLM context for the repo |
| `PROJECT_CONTEXT.md`             | Architecture and design overview |
| `config-validation.md`           | Config validation rules |
| `contributor-verification-guide.md` | SC-W5-120: Wave acceptance steps |
| `contract-module-map.md`         | This file |

---

## Where to Start

- **Bug in SLA logic** → `sla_calculator/src/lib.rs` + `tests.rs`
- **Backend integration issue** → `offchain/` helpers
- **Release or deployment issue** → `tooling/` or `scripts/`
- **Documentation gap** → `docs/`
- **New contributor** → read `docs/contributor-verification-guide.md` first

---

## Contract Entrypoint Map

Every method exported by the `#[contractimpl]` block of `sla_calculator/src/lib.rs`, with where it
lives and who may call it. `scripts/check-module-map.ts` (run in CI) compares this table with
`lib.rs` and fails on a missing or stale method, so **add a row here in the same change that
exports a new method**. Only the first column (the backticked method name) is machine-checked.

| Method | Implementation | Access | Notes |
|--------|----------------|--------|-------|
| `initialize` | lib.rs | Anyone, once only (`AlreadyInitialized` afterwards) | Sets admin, operator and defaults |
| `migrate` | lib.rs | Admin (checked against the stored admin; usable before the version stamp exists) | Idempotent v0 to current step |
| `get_admin` | lib.rs | Public read |  |
| `get_operator` | lib.rs | Public read |  |
| `verify_admin_auth` | lib.rs | Admin (`require_auth` plus role check) | Authorization primitive |
| `set_operator` | lib.rs | Admin | Emits `op_set` and the role audit event |
| `revoke_operator` | lib.rs | Admin |  |
| `renounce_operator` | lib.rs | Operator |  |
| `propose_admin` | lib.rs | Admin | Two-step transfer, step 1 |
| `accept_admin` | lib.rs | Pending admin | Two-step transfer, step 2; proposals expire |
| `cancel_admin_proposal` | lib.rs | Admin |  |
| `get_pending_admin` | lib.rs | Public read | Drops an expired proposal as a side effect |
| `get_pending_operator` | lib.rs | Public read | Drops an expired proposal as a side effect |
| `propose_operator` | lib.rs | Admin | Two-step operator change, step 1 |
| `accept_operator` | lib.rs | Pending operator | Two-step operator change, step 2 |
| `cancel_operator_proposal` | lib.rs | Admin |  |
| `renounce_admin` | lib.rs | Admin | Irreversible |
| `pause` | lib.rs | Admin | Blocks `calculate_sla`; see `pause_action.rs` |
| `unpause` | lib.rs | Admin | See `unpause.rs` |
| `is_paused` | lib.rs | Public read |  |
| `get_pause_info` | lib.rs | Public read |  |
| `set_config` | lib.rs | Admin | Validated against the bounds modules |
| `get_config_update_count` | lib.rs | Public read |  |
| `threshold_to_seconds` | lib.rs | Public (pure helper) |  |
| `export_config_map` | lib.rs | Public read |  |
| `import_config_map` | lib.rs | Admin |  |
| `get_config` | lib.rs | Public read |  |
| `list_configs` | lib.rs | Public read |  |
| `get_config_snapshot` | lib.rs | Public read |  |
| `reset_config_to_defaults` | lib.rs | Admin |  |
| `get_config_version_hash` | lib.rs | Public read |  |
| `compute_result_hash` | lib.rs | Public (pure) |  |
| `get_failure_schema` | lib.rs | Public read |  |
| `get_result_schema` | lib.rs | Public read |  |
| `get_contract_metadata` | lib.rs | Public read |  |
| `get_stats` | lib.rs | Public read |  |
| `simulate_sla` | lib.rs | Public (read-only simulation) |  |
| `calculate_sla_view` | lib.rs | Public (read-only) |  |
| `calculate_monthly_sla_compliance` | lib.rs | Public (read-only) |  |
| `batch_calculate_view` | lib.rs | Public (read-only) |  |
| `batch_calculate` | lib.rs (delegates to `batch.rs`) | Operator | Writes history like `calculate_sla` |
| `get_batch_downtime` | lib.rs | Public (read-only) | Up to 50 site IDs |
| `calculate_sla` | lib.rs | Operator (and not paused) | Writes history, stats and events |
| `get_initialization_status` | lib.rs | Public read |  |
| `get_history_summary` | lib.rs | Public read |  |
| `get_history` | lib.rs | Public read |  |
| `prune_history` | lib.rs | Admin |  |
| `prune_history_by_age` | lib.rs | Admin |  |
| `get_history_page` | lib.rs | Public read |  |
| `get_history_by_outage` | lib.rs | Public read |  |
| `get_latest_by_outage` | lib.rs | Public read |  |
| `get_config_count` | lib.rs | Public read |  |
| `get_storage_version` | lib.rs | Public read |  |
| `set_retention_limit` | lib.rs | Admin |  |
| `get_retention_limit` | lib.rs | Public read |  |
| `set_max_outage_duration` | lib.rs | Admin |  |
| `get_max_outage_duration` | lib.rs | Public read |  |
| `get_migration_state` | lib.rs | Public read |  |
| `get_version_info` | lib.rs | Public read | See `version_negotiation.rs` |
| `set_migration_key` | lib.rs | Admin | Starts the time-locked migration |
| `execute_migration` | lib.rs | Migration key holder | Time-locked |
| `prune_history_queue` | lib.rs | Admin |  |

