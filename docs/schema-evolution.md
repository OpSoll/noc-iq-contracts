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

## Backward Compatibility Rules

- Additive field additions to existing structs are backward compatible
- Field removal or type changes require a version bump
- New error variants must be appended to the end of the enum
- Event schema changes must emit a new event version symbol
