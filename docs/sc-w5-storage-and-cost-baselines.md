# SC-W5 Storage and Cost Baselines

This baseline captures the initial contract posture for:
- storage key namespace collision expectations
- pruning-by-age chronology behavior
- storage footprint telemetry signals
- `calculate_sla` critical-path cost regression checkpoints

## Storage Namespace Canonicalization
- Contract storage keys are Symbol constants in `sla_calculator/src/lib.rs`.
- Canonicalization rule: no duplicate semantic domains across keys.
- Regression guard: test additions should fail on newly introduced collisions.

## Pruning-by-Age Chronology
- Pruning behavior must remain deterministic when event chronology is mixed.
- Baseline expectation: older records are removed first regardless of insertion pattern.

## Storage Footprint Telemetry
- Telemetry surface should track:
  - history length before/after prune
  - retained-count target
  - prune operation cadence

## Critical Path Cost Baseline
- `calculate_sla` is treated as critical path.
- Baseline tests should compare behavior across repeated runs and detect major regressions.
