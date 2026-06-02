/**
 * SC-W5-061: Deterministic simulation harness for threshold edge sweeps.
 * Verifies that sweeping threshold values produces stable, reproducible results.
 */

interface SlaConfig { threshold: number; penaltyBps: number }
type Verdict = "met" | "violated" | "invalid";

function evaluate(mttr: number, cfg: SlaConfig): Verdict {
  if (mttr < 0 || cfg.threshold <= 0) return "invalid";
  return mttr <= cfg.threshold ? "met" : "violated";
}

const THRESHOLDS = [30, 60, 120, 240, 480];

describe("SC-W5-061 Deterministic Simulation Harness", () => {
  it("sweep produces identical results on repeated runs", () => {
    const run = () => THRESHOLDS.map((t) => evaluate(t, { threshold: t, penaltyBps: 100 }));
    expect(run()).toEqual(run());
  });

  it("boundary value at exactly threshold is always met", () => {
    for (const t of THRESHOLDS) {
      expect(evaluate(t, { threshold: t, penaltyBps: 100 })).toBe("met");
    }
  });

  it("one unit above threshold is always violated", () => {
    for (const t of THRESHOLDS) {
      expect(evaluate(t + 1, { threshold: t, penaltyBps: 100 })).toBe("violated");
    }
  });

  it("zero threshold is always invalid regardless of mttr", () => {
    for (const mttr of [0, 1, 100]) {
      expect(evaluate(mttr, { threshold: 0, penaltyBps: 100 })).toBe("invalid");
    }
  });

  it("sweep order does not affect individual verdicts", () => {
    const forward = THRESHOLDS.map((t) => evaluate(60, { threshold: t, penaltyBps: 50 }));
    const reverse = [...THRESHOLDS].reverse().map((t) => evaluate(60, { threshold: t, penaltyBps: 50 }));
    expect(forward).toEqual([...reverse].reverse());
  });
});
