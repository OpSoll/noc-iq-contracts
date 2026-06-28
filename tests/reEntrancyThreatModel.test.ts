import { describe, it, expect } from "vitest";

interface CallCtx { caller: string; method: string; depth: number; }

const MAX_DEPTH = 1;

function guardReentrancy(ctx: CallCtx): void {
  if (ctx.depth > MAX_DEPTH) {
    throw new Error(`Re-entrancy: ${ctx.caller} -> ${ctx.method} at depth ${ctx.depth}`);
  }
}

describe("re-entrancy and unexpected callback threat model", () => {
  it("allows top-level call (depth 0)", () => {
    expect(() => guardReentrancy({ caller: "admin", method: "register_outage", depth: 0 })).not.toThrow();
  });

  it("allows first nested call (depth 1)", () => {
    expect(() => guardReentrancy({ caller: "admin", method: "register_outage", depth: 1 })).not.toThrow();
  });

  it("blocks re-entrant call at depth 2", () => {
    expect(() => guardReentrancy({ caller: "attacker", method: "register_outage", depth: 2 })).toThrow("Re-entrancy");
  });

  it("blocks deeply nested re-entrant call", () => {
    expect(() => guardReentrancy({ caller: "attacker", method: "calc_sla", depth: 5 })).toThrow();
  });
});
