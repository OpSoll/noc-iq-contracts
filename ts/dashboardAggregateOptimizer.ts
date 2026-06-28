export interface AggregateQuery {
  outageIds: string[];
  fields: string[];
  limit?: number;
}

export interface AggregateResult {
  total: number;
  data: Record<string, unknown>[];
  cached: boolean;
}

const cache = new Map<string, AggregateResult>();

function cacheKey(q: AggregateQuery): string {
  return `${[...q.outageIds].sort().join(",")}|${[...q.fields].sort().join(",")}|${q.limit ?? "all"}`;
}

export function runAggregateQuery(q: AggregateQuery, source: Record<string, unknown>[]): AggregateResult {
  const key = cacheKey(q);
  if (cache.has(key)) return { ...cache.get(key)!, cached: true };

  const data = source
    .filter((r) => q.outageIds.includes(r["outageId"] as string))
    .slice(0, q.limit ?? source.length)
    .map((r) => Object.fromEntries(q.fields.map((f) => [f, r[f]])));

  const result: AggregateResult = { total: data.length, data, cached: false };
  cache.set(key, result);
  return result;
}

export function clearAggregateCache(): void { cache.clear(); }
