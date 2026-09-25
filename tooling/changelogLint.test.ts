import { describe, it, expect, afterEach } from "vitest";
import { mkdtempSync, rmSync, writeFileSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import { lintChangelog } from "./changelogLint";

describe("lintChangelog", () => {
  const tmpDirs: string[] = [];

  function changelogFixture(content: string): string {
    const dir = mkdtempSync(join(tmpdir(), "changelog-lint-test-"));
    tmpDirs.push(dir);
    const path = join(dir, "CHANGELOG.md");
    writeFileSync(path, content, "utf8");
    return path;
  }

  afterEach(() => {
    while (tmpDirs.length) {
      rmSync(tmpDirs.pop()!, { recursive: true, force: true });
    }
  });

  it("passes when a new public function has a matching [interface] entry", () => {
    const changelog = changelogFixture(
      "## Unreleased\n- [interface] pub fn migrate\n",
    );
    const diff = "+ pub fn migrate(env: Env) -> Result<(), Error> {";

    const result = lintChangelog(diff, changelog);

    expect(result.ok).toBe(true);
    expect(result.missing).toEqual([]);
  });

  it("fails when a new public function has no changelog entry", () => {
    const changelog = changelogFixture("## Unreleased\n- Some unrelated note\n");
    const diff = "+ pub fn migrate(env: Env) -> Result<(), Error> {";

    const result = lintChangelog(diff, changelog);

    expect(result.ok).toBe(false);
    expect(result.missing).toContain("[interface] pub fn migrate");
  });

  it("passes for a diff with no interface-affecting changes", () => {
    const changelog = changelogFixture("## Unreleased\n");
    const diff = "+ // just a comment tweak";

    const result = lintChangelog(diff, changelog);

    expect(result.ok).toBe(true);
    expect(result.missing).toEqual([]);
  });

  it("flags a new contracttype without a matching entry", () => {
    const changelog = changelogFixture("## Unreleased\n");
    const diff = "+ #[contracttype]\n+ pub struct NewThing {}";

    const result = lintChangelog(diff, changelog);

    expect(result.ok).toBe(false);
    expect(result.missing.some((m) => m.includes("#[contracttype]"))).toBe(
      true,
    );
  });
});
