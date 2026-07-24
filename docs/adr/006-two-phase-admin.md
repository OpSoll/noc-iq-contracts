# ADR-006: Two-Phase Admin Transfer

## Status

Accepted

## Context

The SLA Calculator contract requires a secure mechanism for transferring admin authority. A single-phase transfer (direct assignment) poses security risks:

1. **Accidental transfers**: A typo in the admin address could lock out the contract
2. **No confirmation**: The new admin has no opportunity to accept the role
3. **No rollback**: Once transferred, the transfer cannot be cancelled

## Decision

Implement a two-phase admin transfer protocol:

1. **Proposal Phase**: Current admin proposes a new admin address
2. **Acceptance Phase**: Proposed admin must explicitly accept the role

### Protocol Flow

```
Current Admin                    Proposed Admin
     |                                |
     |-- propose_admin(new_admin) --->|
     |                                |
     |<----------- accept_admin() ---|
     |                                |
     [Admin role transferred]
```

### Implementation

```rust
// Phase 1: Proposal (admin only)
pub fn propose_admin(caller: Address, new_admin: Address) -> Result<(), SLAError>;

// Phase 2: Acceptance (proposed admin only)
pub fn accept_admin(caller: Address) -> Result<(), SLAError>;

// Optional: Cancel pending proposal (admin only)
pub fn cancel_admin_proposal(caller: Address) -> Result<(), SLAError>;

// Query pending proposal
pub fn get_pending_admin() -> Option<Address>;
```

### Security Properties

- **No race conditions**: Proposal is atomic; only one pending at a time
- **Confirmation required**: New admin must explicitly accept
- **Cancellation**: Current admin can cancel before acceptance
- **Clear state**: Pending proposal is cleared after acceptance

## Consequences

### Positive

- **Safety**: Prevents accidental admin transfers
- **Confirmation**: New admin acknowledges role before accepting
- **Reversibility**: Admin can cancel before acceptance
- **Audit trail**: Events emitted for proposal and acceptance

### Negative

- **Complexity**: Two transactions instead of one
- **Latency**: Transfer requires coordination between parties
- **State storage**: Must store pending admin address

### Mitigations

- **Timeout**: Consider adding proposal expiration (future enhancement)
- **Events**: All operations emit events for monitoring
- **Idempotency**: Re-proposing same address is safe

## Alternatives Considered

1. **Single-phase transfer**: Rejected due to safety concerns
2. **Multi-sig admin**: Rejected for complexity; out of scope
3. **Time-locked transfer**: Considered but deferred to future iteration

## References

- Issue #63: Two-step admin transfer
- Soroban documentation: Address authorization
