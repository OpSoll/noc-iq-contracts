export type AdminAddress = string;

export interface QuorumConfig { threshold: number; admins: AdminAddress[]; }

export interface QuorumHookContext {
  action: string;
  signers: AdminAddress[];
  config: QuorumConfig;
}

function validSigners(ctx: QuorumHookContext): AdminAddress[] {
  return ctx.signers.filter((s) => ctx.config.admins.includes(s));
}

export function hasQuorum(ctx: QuorumHookContext): boolean {
  return validSigners(ctx).length >= ctx.config.threshold;
}

export function quorumStatus(ctx: QuorumHookContext): { met: boolean; have: number; need: number } {
  const have = validSigners(ctx).length;
  return { met: have >= ctx.config.threshold, have, need: ctx.config.threshold };
}

// Placeholder hooks for future multi-sig admin wiring
export const onBeforeGovernanceAction = (ctx: QuorumHookContext): void => {
  if (!hasQuorum(ctx)) {
    const s = quorumStatus(ctx);
    throw new Error(`Quorum not met for "${ctx.action}": ${s.have}/${s.need}`);
  }
};

export const onAfterGovernanceAction = (_ctx: QuorumHookContext): void => {
  // reserved for post-action audit emission
};
