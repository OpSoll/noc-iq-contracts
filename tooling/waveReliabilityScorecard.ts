/**
 * SC-W5-125: Wave reliability scorecard generation for contract releases.
 *
 * Aggregates multiple release health signals into a single numeric scorecard
 * (0–100) per Wave release. The scorecard is used by maintainers to assess
 * overall reliability before promoting a release to production.
 */

export interface ReleaseSignals {
  /** Ratio of cargo test cases that passed: 0.0–1.0 */
  test_pass_ratio: number;
  /** Ratio of post-deploy smoke checks that passed: 0.0–1.0 */
  smoke_pass_ratio: number;
  /** Observed backend error rate since deploy: 0.0–1.0 */
  error_rate: number;
  /** WASM size as fraction of budget: 0.0–1.0 (lower is better) */
  wasm_size_fraction: number;
  /** Whether the pre-deploy manifest check passed */
  manifest_ok: boolean;
  /** Whether no uncommitted changes were present at build time */
  clean_build: boolean;
}

export interface ScorecardResult {
  score: number; // 0–100
  grade: "A" | "B" | "C" | "F";
  breakdown: Record<string, number>; // component scores
  recommendation: string;
}

/**
 * Weighted scoring:
 *   test_pass_ratio   30 pts
 *   smoke_pass_ratio  25 pts
 *   error_rate        20 pts (inverted: 0% error = 20 pts)
 *   wasm_size         10 pts (inverted: 0% usage = 10 pts)
 *   manifest_ok        8 pts
 *   clean_build        7 pts
 */
export function generateScorecard(signals: ReleaseSignals): ScorecardResult {
  const breakdown: Record<string, number> = {
    test_pass:    Math.round(signals.test_pass_ratio * 30),
    smoke_pass:   Math.round(signals.smoke_pass_ratio * 25),
    error_rate:   Math.round(Math.max(0, 1 - signals.error_rate) * 20),
    wasm_size:    Math.round(Math.max(0, 1 - signals.wasm_size_fraction) * 10),
    manifest_ok:  signals.manifest_ok ? 8 : 0,
    clean_build:  signals.clean_build ? 7 : 0,
  };

  const score = Object.values(breakdown).reduce((a, b) => a + b, 0);
  const grade: ScorecardResult["grade"] =
    score >= 90 ? "A" : score >= 75 ? "B" : score >= 60 ? "C" : "F";

  const recommendation =
    grade === "A" ? "Promote to production" :
    grade === "B" ? "Promote with monitoring" :
    grade === "C" ? "Hold — address flagged signals" :
                    "Do not promote — critical signals failing";

  return { score, grade, breakdown, recommendation };
}

// Tests
function runTests(): void {
  console.log("[SC-W5-125] Wave reliability scorecard tests\n");

  const perfect: ReleaseSignals = {
    test_pass_ratio: 1.0, smoke_pass_ratio: 1.0, error_rate: 0,
    wasm_size_fraction: 0.5, manifest_ok: true, clean_build: true,
  };
  let sc = generateScorecard(perfect);
  if (sc.grade !== "A") throw new Error(`Expected A, got ${sc.grade} (score=${sc.score})`);
  console.log(`  ✓ perfect signals -> A (${sc.score}/100)`);

  const degraded: ReleaseSignals = {
    ...perfect, smoke_pass_ratio: 0.7, error_rate: 0.05,
  };
  sc = generateScorecard(degraded);
  if (sc.score >= 90) throw new Error("Degraded signals should score below 90");
  console.log(`  ✓ degraded signals -> ${sc.grade} (${sc.score}/100)`);

  const failing: ReleaseSignals = {
    test_pass_ratio: 0.5, smoke_pass_ratio: 0.5, error_rate: 0.2,
    wasm_size_fraction: 0.95, manifest_ok: false, clean_build: false,
  };
  sc = generateScorecard(failing);
  if (sc.grade !== "F") throw new Error(`Expected F, got ${sc.grade}`);
  console.log(`  ✓ failing signals -> F (${sc.score}/100)`);

  // Score bounds
  const allZero: ReleaseSignals = {
    test_pass_ratio: 0, smoke_pass_ratio: 0, error_rate: 1,
    wasm_size_fraction: 1, manifest_ok: false, clean_build: false,
  };
  sc = generateScorecard(allZero);
  if (sc.score !== 0) throw new Error(`Expected 0, got ${sc.score}`);
  console.log("  ✓ all-zero signals -> score=0");

  console.log("\nAll scorecard tests passed.");
}

if (require.main === module) {
  runTests();
}
