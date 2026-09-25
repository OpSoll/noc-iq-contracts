// SC-048: Classify snapshot diffs as expected or suspicious for PR review guidance.
// Reads two snapshot files and reports which changed fields are semantic vs volatile.

import * as fs from "fs";

const VOLATILE = new Set(["timestamp", "generated_at", "elapsed_ms", "run_id"]);

export type JsonVal = string | number | boolean | null | JsonVal[] | { [k: string]: JsonVal };

export function flatten(obj: JsonVal, prefix = ""): Record<string, JsonVal> {
  if (obj === null || typeof obj !== "object") return { [prefix]: obj };
  if (Array.isArray(obj)) {
    return obj.reduce<Record<string, JsonVal>>((acc, v, i) => {
      Object.assign(acc, flatten(v as JsonVal, `${prefix}[${i}]`));
      return acc;
    }, {});
  }
  return Object.entries(obj).reduce<Record<string, JsonVal>>((acc, [k, v]) => {
    Object.assign(acc, flatten(v as JsonVal, prefix ? `${prefix}.${k}` : k));
    return acc;
  }, {});
}

export interface DiffClassification {
  semanticChanges: { key: string; before: JsonVal; after: JsonVal }[];
  volatileChanges: string[];
}

/**
 * Classifies every changed leaf key between two already-parsed JSON
 * snapshots as either "volatile" (known-noisy fields like timestamps) or
 * "semantic" (a real change worth reviewing).
 */
export function classifyDiff(before: JsonVal, after: JsonVal): DiffClassification {
  const a = flatten(before);
  const b = flatten(after);
  const keys = new Set([...Object.keys(a), ...Object.keys(b)]);
  const semanticChanges: { key: string; before: JsonVal; after: JsonVal }[] = [];
  const volatileChanges: string[] = [];

  for (const k of keys) {
    if (a[k] === b[k]) continue;
    const leaf = k.split(".").pop() ?? k;
    if (VOLATILE.has(leaf)) {
      volatileChanges.push(k);
    } else {
      semanticChanges.push({ key: k, before: a[k], after: b[k] });
    }
  }

  return { semanticChanges, volatileChanges };
}

function classify(beforePath: string, afterPath: string): void {
  const a = JSON.parse(fs.readFileSync(beforePath, "utf8"));
  const b = JSON.parse(fs.readFileSync(afterPath, "utf8"));
  const { semanticChanges, volatileChanges } = classifyDiff(a, b);

  for (const change of volatileChanges) {
    console.log(`  [volatile]  ${change}`);
  }
  for (const change of semanticChanges) {
    console.log(
      `  [SEMANTIC]  ${change.key}  ${JSON.stringify(change.before)} → ${JSON.stringify(change.after)}`,
    );
  }

  console.log(
    `\n${semanticChanges.length} semantic change(s), ${volatileChanges.length} volatile change(s).`,
  );
  if (semanticChanges.length === 0 && volatileChanges.length > 0) {
    console.log("Verdict: noise-only diff — safe to ignore.");
  } else if (semanticChanges.length > 0) {
    console.log("Verdict: real change detected — review required.");
  }
}

if (require.main === module) {
  const [, , before, after] = process.argv;
  if (!before || !after) {
    console.error("Usage: ts-node diff-snapshots.ts <before.json> <after.json>");
    process.exit(1);
  }
  classify(before, after);
}
