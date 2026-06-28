export interface GovernanceAction {
  id: string;
  type: string;
  payload: unknown;
  executedAt: number;
}

function hashAction(a: GovernanceAction): string {
  return `${a.type}:${JSON.stringify(a.payload)}:${a.executedAt}`;
}

export class ReplayDetector {
  private seen = new Set<string>();

  isReplay(action: GovernanceAction): boolean {
    return this.seen.has(hashAction(action));
  }

  record(action: GovernanceAction): void {
    const h = hashAction(action);
    if (this.seen.has(h)) throw new Error(`Replay detected for action ${action.id}`);
    this.seen.add(h);
  }

  clear(): void { this.seen.clear(); }

  size(): number { return this.seen.size; }
}
