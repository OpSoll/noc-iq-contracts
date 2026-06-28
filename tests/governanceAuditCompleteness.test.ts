import { describe, it, expect } from "vitest";

type GovernanceAction = "freeze_config" | "unfreeze_config" | "rotate_admin" | "update_quorum";

interface AuditEvent { action: GovernanceAction; actor: string; timestamp: number; txHash: string; }

const requiredActions: GovernanceAction[] = ["freeze_config", "unfreeze_config", "rotate_admin", "update_quorum"];

function missingActions(events: AuditEvent[]): GovernanceAction[] {
  const emitted = new Set(events.map((e) => e.action));
  return requiredActions.filter((a) => !emitted.has(a));
}

const partial: AuditEvent[] = [
  { action: "freeze_config", actor: "admin1", timestamp: 1_700_000_000, txHash: "0xaaa" },
  { action: "rotate_admin",  actor: "admin1", timestamp: 1_700_000_001, txHash: "0xbbb" },
];

describe("on-chain governance audit event completeness", () => {
  it("detects missing audit events in a partial set", () => {
    const missing = missingActions(partial);
    expect(missing).toContain("unfreeze_config");
    expect(missing).toContain("update_quorum");
  });

  it("reports empty when all required events are present", () => {
    const full = requiredActions.map((a, i) => ({ action: a, actor: "admin", timestamp: i, txHash: `0x${i}` }));
    expect(missingActions(full)).toHaveLength(0);
  });
});
