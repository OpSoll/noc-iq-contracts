export interface PendingConfigChange {
  key: string;
  newValue: unknown;
  proposedAt: number;
  activateAfterMs: number;
  activatedAt?: number;
}

export class TimedConfigActivator {
  private pending: PendingConfigChange[] = [];

  propose(key: string, newValue: unknown, delayMs: number): void {
    this.pending.push({ key, newValue, proposedAt: Date.now(), activateAfterMs: delayMs });
  }

  readyToActivate(now = Date.now()): PendingConfigChange[] {
    return this.pending.filter((c) => !c.activatedAt && now - c.proposedAt >= c.activateAfterMs);
  }

  activate(now = Date.now()): PendingConfigChange[] {
    const ready = this.readyToActivate(now);
    for (const c of ready) c.activatedAt = now;
    return ready;
  }

  pendingCount(): number { return this.pending.filter((c) => !c.activatedAt).length; }
}
