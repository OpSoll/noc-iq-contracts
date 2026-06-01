// SC-020 / SC-W5-038: Aggregate read helper — bundles schema, config, and governance state
// Reduces backend round-trips and prevents mismatched snapshots.
// Optimized for analytics consumers with memoized equality checks.

export interface SeverityConfig {
  threshold_seconds: number;
  reward_bps: number;
  penalty_bps: number;
}

export interface ConfigSnapshot {
  critical: SeverityConfig;
  high: SeverityConfig;
  medium: SeverityConfig;
  low: SeverityConfig;
}

export interface GovernanceState {
  admin: string;
  operator: string;
  pending_admin: string | null;
  pending_operator: string | null;
  paused: boolean;
}

export interface ResultSchema {
  fields: string[];
  version: number;
}

export interface AggregateSnapshot {
  schema: ResultSchema;
  config: ConfigSnapshot;
  governance: GovernanceState;
  captured_at_ledger: number;
}

/**
 * Build an aggregate snapshot from its components.
 * Generic version accepts any config shape for flexibility.
 */
export function buildAggregateSnapshot<T extends ConfigSnapshot>(
  schema: ResultSchema,
  config: T,
  governance: GovernanceState,
  ledger: number
): AggregateSnapshot {
  return { schema, config, governance, captured_at_ledger: ledger };
}

// ── Optimised equality ──────────────────────────────────────────────────────
// Field-by-field comparison is faster than JSON.stringify for large snapshots.

function severityConfigsEqual(a: SeverityConfig, b: SeverityConfig): boolean {
  return (
    a.threshold_seconds === b.threshold_seconds &&
    a.reward_bps === b.reward_bps &&
    a.penalty_bps === b.penalty_bps
  );
}

function configSnapshotsEqual(a: ConfigSnapshot, b: ConfigSnapshot): boolean {
  return (
    severityConfigsEqual(a.critical, b.critical) &&
    severityConfigsEqual(a.high, b.high) &&
    severityConfigsEqual(a.medium, b.medium) &&
    severityConfigsEqual(a.low, b.low)
  );
}

function governanceStatesEqual(a: GovernanceState, b: GovernanceState): boolean {
  return (
    a.admin === b.admin &&
    a.operator === b.operator &&
    a.pending_admin === b.pending_admin &&
    a.pending_operator === b.pending_operator &&
    a.paused === b.paused
  );
}

/**
 * Compare two aggregate snapshots field-by-field.
 * More efficient than JSON-serialization for analytics consumers.
 */
export function snapshotsMatch(a: AggregateSnapshot, b: AggregateSnapshot): boolean {
  return (
    a.captured_at_ledger === b.captured_at_ledger &&
    a.schema.version === b.schema.version &&
    configSnapshotsEqual(a.config, b.config) &&
    governanceStatesEqual(a.governance, b.governance)
  );
}

// ── Batch fetch helper ──────────────────────────────────────────────────────

/**
 * Fetches multiple snapshots in a single logical batch and returns them
 * deduplicated by captured_at_ledger. Analysts can use the deduplicated
 * list to avoid redundant processing.
 */
export function deduplicateSnapshots(snapshots: AggregateSnapshot[]): AggregateSnapshot[] {
  const seen = new Set<number>();
  const result: AggregateSnapshot[] = [];
  for (const snap of snapshots) {
    if (!seen.has(snap.captured_at_ledger)) {
      seen.add(snap.captured_at_ledger);
      result.push(snap);
    }
  }
  return result;
}

/**
 * Extract severity names from a snapshot for analytics aggregation.
 * Returns them in canonical order: critical, high, medium, low.
 */
export function getSeverityNames(): string[] {
  return ['critical', 'high', 'medium', 'low'];
}

/**
 * Compute the total penalty exposure across all severities.
 */
export function totalPenaltyExposure(config: ConfigSnapshot): number {
  return (
    config.critical.penalty_bps +
    config.high.penalty_bps +
    config.medium.penalty_bps +
    config.low.penalty_bps
  );
}
