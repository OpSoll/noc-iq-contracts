interface CalcInput  { outageId: string; duration: number; severity: number; }
interface CalcResult { outageId: string; sla: number; durationMs: number; }

function calcSla(inp: CalcInput): CalcResult {
  const start = Date.now();
  const sla   = Math.max(0, 100 - inp.severity * (inp.duration / 3_600));
  return { outageId: inp.outageId, sla: Math.round(sla * 100) / 100, durationMs: Date.now() - start };
}

export async function runThroughputStress(count: number, concurrency: number): Promise<void> {
  const inputs: CalcInput[] = Array.from({ length: count }, (_, i) => ({
    outageId: `OUTAGE-${i.toString().padStart(5, "0")}`,
    duration: 1_800 + (i % 7_200),
    severity: (i % 5) + 1,
  }));

  const start = Date.now();
  let processed = 0;

  for (let i = 0; i < inputs.length; i += concurrency) {
    const batch   = inputs.slice(i, i + concurrency);
    const results = await Promise.all(batch.map((inp) => Promise.resolve(calcSla(inp))));
    processed    += results.length;
  }

  const elapsed = Date.now() - start;
  const opsPerSec = Math.round(processed / (elapsed / 1_000));
  console.log(`Throughput stress: ${processed} calcs in ${elapsed}ms (${opsPerSec} ops/sec)`);
}

runThroughputStress(1_000, 50).catch(console.error);
