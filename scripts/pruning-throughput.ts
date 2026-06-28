interface PruneRecord { id: string; age: number; size: number; }

interface PruneResult {
  pruned: number;
  retained: number;
  bytesFreed: number;
  durationMs: number;
}

export function pruneByAge(records: PruneRecord[], maxAgeDays: number): PruneResult {
  const start  = Date.now();
  const cutoff = maxAgeDays * 86_400_000;
  const now    = Date.now();
  let pruned = 0, bytesFreed = 0, retained = 0;

  for (const r of records) {
    if (now - r.age > cutoff) { pruned++; bytesFreed += r.size; }
    else retained++;
  }

  return { pruned, retained, bytesFreed, durationMs: Date.now() - start };
}

export function logPruneResult(r: PruneResult): void {
  console.log(
    `Pruned ${r.pruned} records (${r.bytesFreed} bytes) in ${r.durationMs}ms. Retained: ${r.retained}.`
  );
}
