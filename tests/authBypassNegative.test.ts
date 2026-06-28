import { describe, it, expect } from "vitest";

type Role = "admin" | "operator" | "observer" | "anonymous";
interface AuthCtx { role: Role; address: string; }

const privilegedPaths = ["freeze_config", "rotate_admin", "emit_governance_event", "force_close_outage"];

function isAuthorized(ctx: AuthCtx, path: string): boolean {
  if (path === "force_close_outage")   return ctx.role === "admin";
  if (path === "rotate_admin")         return ctx.role === "admin";
  if (path === "emit_governance_event")return ctx.role === "admin";
  if (path === "freeze_config")        return ctx.role === "admin" || ctx.role === "operator";
  return false;
}

describe("authorization bypass negative tests", () => {
  const anon:     AuthCtx = { role: "anonymous", address: "GANON" };
  const observer: AuthCtx = { role: "observer",  address: "GOBS"  };

  for (const path of privilegedPaths) {
    it(`anonymous cannot access ${path}`,() => expect(isAuthorized(anon, path)).toBe(false));
    it(`observer cannot access ${path}`, () => expect(isAuthorized(observer, path)).toBe(false));
  }
});
