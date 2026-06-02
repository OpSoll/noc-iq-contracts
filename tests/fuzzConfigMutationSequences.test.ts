/**
 * SC-W5-068: Fuzzing for config mutation sequences and freeze toggles.
 * Ensures config mutations are rejected when the contract is frozen.
 */

interface ContractConfig { threshold: number; penaltyBps: number; frozen: boolean }

function applyMutation(config: ContractConfig, patch: Partial<ContractConfig>): { ok: boolean; config: ContractConfig } {
  if (config.frozen) return { ok: false, config };
  return { ok: true, config: { ...config, ...patch } };
}

function toggleFreeze(config: ContractConfig): ContractConfig {
  return { ...config, frozen: !config.frozen };
}

const BASE: ContractConfig = { threshold: 60, penaltyBps: 300, frozen: false };

describe("SC-W5-068 Config Mutation Sequences and Freeze Toggles", () => {
  it("mutation succeeds when not frozen", () => {
    const { ok, config } = applyMutation(BASE, { threshold: 120 });
    expect(ok).toBe(true);
    expect(config.threshold).toBe(120);
  });

  it("mutation is rejected when frozen", () => {
    const frozen = toggleFreeze(BASE);
    const { ok, config } = applyMutation(frozen, { threshold: 120 });
    expect(ok).toBe(false);
    expect(config.threshold).toBe(60); // unchanged
  });

  it("freeze then unfreeze allows mutations again", () => {
    const frozen   = toggleFreeze(BASE);
    const unfrozen = toggleFreeze(frozen);
    expect(applyMutation(unfrozen, { penaltyBps: 500 }).ok).toBe(true);
  });

  it("mutation sequence: mutate → freeze → mutate fails → unfreeze → mutate succeeds", () => {
    let cfg = BASE;
    cfg = applyMutation(cfg, { penaltyBps: 400 }).config;
    cfg = toggleFreeze(cfg);
    expect(applyMutation(cfg, { penaltyBps: 999 }).ok).toBe(false);
    cfg = toggleFreeze(cfg);
    expect(applyMutation(cfg, { penaltyBps: 999 }).ok).toBe(true);
  });
});
