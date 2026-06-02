/**
 * SC-W5-066: Property tests for MTTR monotonicity and payout gradients.
 * Higher MTTR should never produce a lower penalty (monotone non-decreasing).
 */

function computePenalty(mttr: number, threshold: number, penaltyBps: number): number {
  if (mttr <= threshold || threshold <= 0) return 0;
  return Math.floor((mttr - threshold) * penaltyBps / 10000);
}

function computePayout(penaltyBps: number, contractValue: number): number {
  return Math.floor(contractValue * penaltyBps / 10000);
}

const THRESHOLD = 60;
const BPS = 500;
const CONTRACT_VALUE = 1_000_000;

describe("SC-W5-066 MTTR Monotonicity and Payout Gradients", () => {
  it("penalty is non-decreasing as mttr increases", () => {
    const mttrValues = [61, 90, 120, 240, 480];
    let prev = computePenalty(mttrValues[0], THRESHOLD, BPS);
    for (const mttr of mttrValues.slice(1)) {
      const curr = computePenalty(mttr, THRESHOLD, BPS);
      expect(curr).toBeGreaterThanOrEqual(prev);
      prev = curr;
    }
  });

  it("penalty at threshold is zero (boundary)", () => {
    expect(computePenalty(THRESHOLD, THRESHOLD, BPS)).toBe(0);
  });

  it("penalty below threshold is always zero", () => {
    for (const mttr of [0, 1, 30, 59]) {
      expect(computePenalty(mttr, THRESHOLD, BPS)).toBe(0);
    }
  });

  it("payout gradient is positive for non-zero penalty", () => {
    const penalty1 = computePenalty(90,  THRESHOLD, BPS);
    const penalty2 = computePenalty(120, THRESHOLD, BPS);
    expect(computePayout(penalty2, CONTRACT_VALUE)).toBeGreaterThanOrEqual(computePayout(penalty1, CONTRACT_VALUE));
  });
});
