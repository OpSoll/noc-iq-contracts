import { describe, it, expect } from "vitest";

type ConfigState = "active" | "frozen";
interface Config { key: string; value: unknown; state: ConfigState; }

function freezeConfig(c: Config): Config {
  if (c.state !== "active") throw new Error(`Cannot freeze config in state: ${c.state}`);
  return { ...c, state: "frozen" };
}

function unfreezeConfig(c: Config): Config {
  if (c.state !== "frozen") throw new Error(`Cannot unfreeze config in state: ${c.state}`);
  return { ...c, state: "active" };
}

describe("config freeze governance lifecycle completeness", () => {
  const base: Config = { key: "max_severity", value: 5, state: "active" };

  it("freezes an active config", ()   => expect(freezeConfig(base).state).toBe("frozen"));
  it("unfreezes a frozen config", ()  => expect(unfreezeConfig(freezeConfig(base)).state).toBe("active"));
  it("cannot freeze a frozen config", ()  => expect(() => freezeConfig(freezeConfig(base))).toThrow());
  it("cannot unfreeze an active config", ()=> expect(() => unfreezeConfig(base)).toThrow());
});
