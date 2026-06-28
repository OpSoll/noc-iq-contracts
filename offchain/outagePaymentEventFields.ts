export interface OutageEvent {
  outageId: string;
  region: string;
  startedAt: number;
  resolvedAt?: number;
  correlationId: string;
}

export interface PaymentEvent {
  outageId: string;
  amount: number;
  currency: string;
  initiatedAt: number;
  correlationId: string;
}

export function buildCorrelationId(outageId: string, region: string, ts = Date.now()): string {
  return `${region}-${outageId}-${ts}`;
}

export function linkPaymentToOutage(outage: OutageEvent, payment: PaymentEvent): boolean {
  return outage.outageId === payment.outageId && outage.correlationId === payment.correlationId;
}

export function validateTraceFields(event: OutageEvent | PaymentEvent): string[] {
  const missing: string[] = [];
  if (!event.outageId)      missing.push("outageId");
  if (!event.correlationId) missing.push("correlationId");
  return missing;
}
