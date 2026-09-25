import { describe, it, expect } from "vitest";
import { generateTypeDefinitions } from "../src/typegen";

describe("generateTypeDefinitions", () => {
  it("generates a valid-looking interface declaration for a struct", () => {
    const output = generateTypeDefinitions({
      structs: [
        {
          name: "SampleConfig",
          fields: [
            { name: "threshold_minutes", tsType: "number" },
            { name: "reward_base", tsType: "bigint" },
          ],
        },
      ],
    });

    expect(output).toContain("export interface SampleConfig {");
    expect(output).toContain("threshold_minutes: number;");
    expect(output).toContain("reward_base: bigint;");
    expect(output).toContain("}");
  });

  it("generates a camelCase function declaration from a snake_case name", () => {
    const output = generateTypeDefinitions({
      functions: [
        {
          name: "get_config",
          params: [{ name: "severity", tsType: "string" }],
          returns: "SampleConfig",
        },
      ],
    });

    expect(output).toContain(
      "export declare function getConfig(severity: string): Promise<SampleConfig>;",
    );
  });

  it("returns a header comment and stays well-formed with an empty spec", () => {
    const output = generateTypeDefinitions({});
    expect(output).toContain("AUTO-GENERATED");
    expect(output.endsWith("\n")).toBe(true);
  });

  it("handles multiple structs and functions together", () => {
    const output = generateTypeDefinitions({
      structs: [
        { name: "A", fields: [{ name: "x", tsType: "number" }] },
        { name: "B", fields: [{ name: "y", tsType: "string" }] },
      ],
      functions: [
        { name: "do_thing", params: [], returns: "void" },
        {
          name: "do_other_thing",
          params: [{ name: "id", tsType: "string" }],
          returns: "A",
        },
      ],
    });

    expect(output).toContain("export interface A {");
    expect(output).toContain("export interface B {");
    expect(output).toContain(
      "export declare function doThing(): Promise<void>;",
    );
    expect(output).toContain(
      "export declare function doOtherThing(id: string): Promise<A>;",
    );
  });
});
