export interface AdminRotationEvent {
  outgoing: string;
  incoming: string;
  authorizedBy: string;
  rotatedAt: number;
  txHash: string;
}

export interface AuditResult { valid: boolean; issues: string[]; }

export function auditRotation(e: AdminRotationEvent): AuditResult {
  const issues: string[] = [];
  if (!e.outgoing)              issues.push("Missing outgoing admin address");
  if (!e.incoming)              issues.push("Missing incoming admin address");
  if (e.outgoing === e.incoming)issues.push("Outgoing and incoming admin are identical");
  if (!e.authorizedBy)          issues.push("Missing authorizer");
  if (!e.txHash)                issues.push("Missing transaction hash");
  if (e.rotatedAt <= 0)         issues.push("Invalid rotation timestamp");
  return { valid: issues.length === 0, issues };
}

export function auditRotationLog(events: AdminRotationEvent[]): void {
  console.log(`Admin Rotation Audit — ${events.length} event(s)`);
  for (const e of events) {
    const { valid, issues } = auditRotation(e);
    console.log(`  [${valid ? "OK  " : "FAIL"}] ${e.outgoing} → ${e.incoming} by ${e.authorizedBy}`);
    for (const i of issues) console.log(`    ! ${i}`);
  }
}
