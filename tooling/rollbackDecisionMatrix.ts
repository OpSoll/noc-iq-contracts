/**
 * SC-W5-124: Release rollback decision matrix with measurable criteria.
 *
 * Provides a programmatic decision matrix that evaluates post-deploy health
 * signals and returns a structured rollback recommendation: HOLD, MONITOR,
 * or ROLLBACK. Backend operators feed real signal values; this module
 * applies deterministic thresholds to produce a consistent recommendation.
 */

export type RollbackDecision = "HOLD" | "MONITOR" | "ROLLBACK";

export interface DeploySignals {
  /** Smoke checks passed ratio: 0.0–1.0 */
  smoke_pass_ratio: number;
  /** Error rate in backend calls to the contract since deploy: 0.0–1.0 */
  error_rate: number;
  /** Whether the contract is unexpectedly paused post-deploy */
  unexpected_pause: boolean;
  /** Config snapshot matches expected version */
  config_version_ok: boolean;
  /** Minutes since deployment */
  minutes_since_deploy: number;
}

export interface RollbackAssessment {
  decision: RollbackDecision;
  reasons: string[];
  auto_rollback: boolean;
}

const THRESHOLDS = {
  smoke_pass_ratio_min: 1.0,    // all smoke checks must pass
  error_rate_critical: 0.10,    // >10% = rollback
  error_rate_warn: 0.02,        // >2% = monitor
  monitor_window_minutes: 15,   // monitor for 15 min before escalating
};

export function assessRollback(signals: DeploySignals): RollbackAssessment {
  const reasons: string[] = [];
  let decision: RollbackDecision = "HOLD";

  if (signals.unexpected_pause) {
    reasons.push("contract is unexpectedly paused post-deploy");
    decision = "ROLLBACK";
  }

  if (!signals.config_version_ok) {
    reasons.push("config snapshot version does not match expected");
    decision = "ROLLBACK";
  }

  if (signals.smoke_pass_ratio < THRESHOLDS.smoke_pass_ratio_min) {
    reasons.push(`smoke pass ratio ${(signals.smoke_pass_ratio * 100).toFixed(0)}% < 100%`);
    if (decision !== "ROLLBACK") decision = "ROLLBACK";
  }

  if (signals.error_rate > THRESHOLDS.error_rate_critical) {
    reasons.push(`error rate ${(signals.error_rate * 100).toFixed(1)}% exceeds critical threshold (10%)`);
    if (decision !== "ROLLBACK") decision = "ROLLBACK";
  } else if (signals.error_rate > THRESHOLDS.error_rate_warn) {
    reasons.push(`error rate ${(signals.error_rate * 100).toFixed(1)}% above warn threshold (2%)`);
    if (decision === "HOLD") decision = "MONITOR";
  }

  if (decision === "MONITOR" && signals.minutes_since_deploy > THRESHOLDS.monitor_window_minutes) {
    reasons.push(`error rate elevated beyond ${THRESHOLDS.monitor_window_minutes}m monitor window`);
    decision = "ROLLBACK";
  }

  return {
    decision,
    reasons: reasons.length ? reasons : ["all signals nominal"],
    auto_rollback: decision === "ROLLBACK",
  };
}

// Tests
function runTests(): void {
  console.log("[SC-W5-124] Rollback decision matrix tests\n");

  const healthy: DeploySignals = {
    smoke_pass_ratio: 1.0, error_rate: 0.001,
    unexpected_pause: false, config_version_ok: true, minutes_since_deploy: 5,
  };
  let r = assessRollback(healthy);
  if (r.decision !== "HOLD") throw new Error(`Expected HOLD, got ${r.decision}`);
  console.log("  ✓ healthy signals -> HOLD");

  const warnSignal: DeploySignals = { ...healthy, error_rate: 0.05, minutes_since_deploy: 5 };
  r = assessRollback(warnSignal);
  if (r.decision !== "MONITOR") throw new Error(`Expected MONITOR, got ${r.decision}`);
  console.log("  ✓ elevated error rate -> MONITOR");

  const warnExpired: DeploySignals = { ...warnSignal, minutes_since_deploy: 20 };
  r = assessRollback(warnExpired);
  if (r.decision !== "ROLLBACK") throw new Error(`Expected ROLLBACK after monitor window`);
  console.log("  ✓ elevated error rate past monitor window -> ROLLBACK");

  const criticalError: DeploySignals = { ...healthy, error_rate: 0.15 };
  r = assessRollback(criticalError);
  if (r.decision !== "ROLLBACK") throw new Error(`Expected ROLLBACK on critical error rate`);
  console.log("  ✓ critical error rate -> ROLLBACK");

  const pausedSignal: DeploySignals = { ...healthy, unexpected_pause: true };
  r = assessRollback(pausedSignal);
  if (r.decision !== "ROLLBACK" || !r.auto_rollback) throw new Error("Expected ROLLBACK+auto on unexpected pause");
  console.log("  ✓ unexpected pause -> ROLLBACK (auto)");

  console.log("\nAll rollback decision matrix tests passed.");
}

if (require.main === module) {
  runTests();
}
