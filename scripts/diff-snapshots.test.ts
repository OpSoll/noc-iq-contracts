import { describe, it, expect } from "vitest";
import { flatten, classifyDiff } from "./diff-snapshots";

describe("flatten", () => {
  it("flattens nested objects with dot-separated keys", () => {
    expect(flatten({ a: { b: 1, c: 2 } })).toEqual({
      "a.b": 1,
      "a.c": 2,
    });
  });

  it("flattens arrays with bracket-indexed keys", () => {
    expect(flatten({ items: [1, 2] })).toEqual({
      "items[0]": 1,
      "items[1]": 2,
    });
  });

  it("returns a single entry for a primitive value", () => {
    expect(flatten(42, "count")).toEqual({ count: 42 });
  });
});

describe("classifyDiff", () => {
  it("classifies known-volatile fields separately from real changes", () => {
    const before = { timestamp: 1000, threshold_minutes: 15 };
    const after = { timestamp: 2000, threshold_minutes: 30 };

    const result = classifyDiff(before, after);

    expect(result.volatileChanges).toEqual(["timestamp"]);
    expect(result.semanticChanges).toEqual([
      { key: "threshold_minutes", before: 15, after: 30 },
    ]);
  });

  it("reports no changes when snapshots are identical", () => {
    const snapshot = { threshold_minutes: 15 };
    const result = classifyDiff(snapshot, snapshot);

    expect(result.semanticChanges).toEqual([]);
    expect(result.volatileChanges).toEqual([]);
  });

  it("treats an added or removed key as a semantic change", () => {
    const before = { a: 1 };
    const after = { a: 1, b: 2 };

    const result = classifyDiff(before, after);

    expect(result.semanticChanges).toEqual([
      { key: "b", before: undefined, after: 2 },
    ]);
  });

  it("classifies nested volatile fields by their leaf name", () => {
    const before = { meta: { generated_at: "2024-01-01" } };
    const after = { meta: { generated_at: "2024-01-02" } };

    const result = classifyDiff(before, after);

    expect(result.volatileChanges).toEqual(["meta.generated_at"]);
    expect(result.semanticChanges).toEqual([]);
  });
});
