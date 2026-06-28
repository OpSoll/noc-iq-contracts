export type SloStatus = "healthy" | "degraded" | "breached";

export interface ExecutionSloSignal {
  contractId: string;
  method: string;
  p50Ms: number;
  p95Ms: number;
  p99Ms: number;
  errorRate: number;
}

export interface SloThresholds { p95WarnMs: number; p99BreachMs: number; errorRateWarn: number; }

const defaultThresholds: SloThresholds = { p95WarnMs: 500, p99BreachMs: 2_000, errorRateWarn: 0.01 };

export function evaluateSlo(signal: ExecutionSloSignal, t = defaultThresholds): SloStatus {
  if (signal.p99Ms > t.p99BreachMs || signal.errorRate > t.errorRateWarn * 5) return "breached";
  if (signal.p95Ms > t.p95WarnMs   || signal.errorRate > t.errorRateWarn)     return "degraded";
  return "healthy";
}

export function printSloReport(signals: ExecutionSloSignal[]): void {
  for (const s of signals) {
    const status = evaluateSlo(s);
    console.log(`  [${status.toUpperCase().padEnd(8)}] ${s.contractId}::${s.method}  p95=${s.p95Ms}ms p99=${s.p99Ms}ms errRate=${(s.errorRate * 100).toFixed(2)}%`);
  }
}
