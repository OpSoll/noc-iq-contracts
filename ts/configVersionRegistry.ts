export interface ConfigVersion {
  hash: string;
  version: number;
  createdAt: number;
  immutable: boolean;
}

export class ConfigVersionRegistry {
  private registry: ConfigVersion[] = [];

  register(hash: string): ConfigVersion {
    const existing = this.registry.find((v) => v.hash === hash);
    if (existing) return existing;
    const entry: ConfigVersion = { hash, version: this.registry.length + 1, createdAt: Date.now(), immutable: false };
    this.registry.push(entry);
    return entry;
  }

  freeze(hash: string): void {
    const e = this.registry.find((v) => v.hash === hash);
    if (!e) throw new Error(`Hash not registered: ${hash}`);
    e.immutable = true;
  }

  isFrozen(hash: string): boolean { return this.registry.find((v) => v.hash === hash)?.immutable ?? false; }

  latest(): ConfigVersion | undefined { return this.registry.at(-1); }
}
