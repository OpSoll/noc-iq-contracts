# Schema Evolution Playbook

This document describes how to safely evolve the SLA Calculator contract's storage schema and ABI.

## Version Bump Checklist

1. Increment `STORAGE_VERSION` in `lib.rs`
2. Add migration step in `migrate()` function
3. Run the executable migration checks: `cargo test -- migration`
4. Update `CHANGELOG.md` with breaking changes
5. Verify `get_migration_state()` reports correct versions

## Executable Migration Checks

```bash
# Verify migration from any previous version
cargo test --test integration -- migration_upgrade_path

# Verify downgrade safety
cargo test --test integration -- migration_downgrade_guard

# Verify idempotency
cargo test --test integration -- migration_idempotent
```

## Migration Dry Run

`scripts/migration-dry-run.ts` validates, before anything is planned or applied,
that the target schema version is reachable from the deployed version through
the upgrade steps listed in `SUPPORTED_UPGRADES` (keep it in sync with
`sla_calculator/src/storage_migration.rs`). Downgrades, versions newer than
`CURRENT_STORAGE_VERSION`, and jumps with no supported path fail first. The
report lists the exact source version, target version and upgrade path.

## Backward Compatibility Rules

- Additive field additions to existing structs are backward compatible
- Field removal or type changes require a version bump
- New error variants must be appended to the end of the enum
- Event schema changes must emit a new event version symbol

## Config Schema Compatibility Categories

`check_schema_compatibility` in `sla_calculator/src/config_compatibility_check.rs` returns a
`ConfigCompatibilityResult` with a stable `CompatibilityCategory` and the `field_path` that caused
the result:

| Change | Category | Compatible |
|---|---|---|
| No relevant change | `Compatible` | yes |
| Optional field added | `OptionalAddition` | yes |
| Required field added | `BreakingAddition` | no |
| Required field removed | `BreakingRemoval` | no |
| Field type changed | `IncompatibleType` | no |

Breaking findings take precedence over optional additions when a fixture contains both. The
existing version-based `check_config_compatibility` still reports `CompatibilityStatus` to clients.

## Supported Legacy Decode Window

How far back the current contract binary and the off-chain consumers can still read, and what
happens beyond it. Values below are the ones in the code today; update this section in the same
change that bumps any of them.

| Version | Current value | Defined in |
|---|---|---|
| Storage schema (`STORAGE_VERSION`) | `1` | `sla_calculator/src/lib.rs`; mirrored by `EXPECTED_STORAGE_VERSION` in `storage_version.rs`, `STORAGE_VERSION` in `version_discovery.rs` and `CURRENT_STORAGE_VERSION` in `storage_migration.rs` |
| Result schema (`RESULT_SCHEMA_VERSION`) | `1` | `sla_calculator/src/lib.rs` (reported by `get_version_info`) |
| Event version (`EVENT_VERSION`) | `"v1"` | `sla_calculator/src/event_schema.rs` (topic position 1 of every event) |

### Storage: a window of one version

The contract **reads exactly the current storage version**. There is no multi-version read path:

- Every public method starts with `check_version`, which returns `SLAError::VersionMismatch` when the
  stamped version is not equal to `STORAGE_VERSION`. A deployment whose stored version is older
  therefore rejects all calls until it is migrated.
- `assert_storage_version` (`storage_version.rs`) treats an **unset** version as current (a fresh
  contract) and any other value that differs, older or newer, as `VersionMismatch`.
- The only legacy layout the code can upgrade is **version 0** (an unstamped deployment).
  `migrate` stamps it to version 1; it is idempotent at the current version and refuses a stored
  version **newer** than the binary knows (`VersionMismatch`), so a rollback to an older binary
  fails closed instead of misreading newer state.
- `migrate_storage` (`storage_migration.rs`) runs the sequential steps up to
  `CURRENT_STORAGE_VERSION` and likewise rejects a newer on-chain version.

So today: **1 supported version (the current one) plus an unstamped v0 that `migrate` upgrades**.
After the window, that is for any other stored version, the contract does not decode state at all;
it returns `VersionMismatch` and the operator must migrate (older) or deploy a newer binary (newer).

### Events: current version only

Every event carries `EVENT_VERSION` (`"v1"`) as topic[1]. No older event version has been emitted by
a deployed contract, so **there is no legacy event decode window yet**: consumers should treat any
version other than `v1` as unsupported and report it (with its topic and version) rather than
mapping it to a current event. When a breaking event change ships as `v2`, list `v1` here as the
supported legacy version and say for how long. Additive trailing fields do not need a version bump
(see "Schema Versioning" in `event_schema.rs`).

### Migration prerequisites

Before moving a deployment from schema version *N* to *N + 1*:

1. The target version is reachable from the deployed one through `SUPPORTED_UPGRADES` in
   `scripts/migration-dry-run.ts` (currently empty: only the v0 stamp exists) and the matching step
   exists in `storage_migration.rs` and in `migrate()`.
2. `get_migration_state()` reports the expected stored version and `get_version_info()` reports
   `needs_migration` as expected.
3. The caller is the admin (`migrate` is admin-only) and the contract is in a state where a
   brief `VersionMismatch` outage for other callers is acceptable, because calls fail between the
   binary upgrade and the `migrate` transaction.
4. `scripts/migration-dry-run.ts` has been run for the target network and its report reviewed.

### Examples

The results below are the real behaviour of `storage_version.rs` (see its unit tests) and of
`check_version` / `migrate` in `lib.rs`. `check_version` runs first in every public method.

| Stored version | Binary `STORAGE_VERSION` | `assert_storage_version` | Public method (`check_version`) | `migrate` (admin) |
|---|---|---|---|---|
| unset (fresh contract) | 1 | `Ok(1)` | `NotInitialized` until `initialize` stamps it | treated as v0: stamps to 1 once an admin exists |
| 1 | 1 | `Ok(1)` | accepted | no-op, `Ok` |
| 0 (unstamped) | 1 | `VersionMismatch` | `VersionMismatch` | stamps to 1, `Ok` |
| 99 | 1 | `VersionMismatch` | `VersionMismatch` | `VersionMismatch` (newer than the binary) |

Off-chain clients should read `get_version_info()` (storage and result schema versions) before
decoding state, and treat `storage_version` or `result_schema_version` higher than the version
they were built for as "cannot decode safely".

