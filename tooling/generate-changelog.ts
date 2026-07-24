/**
 * generate-changelog.ts
 * Issue #400 — Automated CHANGELOG generation from conventional commits.
 *
 * Parses git log and generates a structured CHANGELOG.md with sections
 * for features, fixes, breaking changes, and other conventional commit types.
 *
 * Usage:
 *   npx ts-node tooling/generate-changelog.ts [--from TAG] [--to TAG] [--output PATH]
 */

import { execSync } from "child_process";
import { writeFileSync, readFileSync, existsSync } from "fs";

/** Changelog section configuration. */
interface SectionConfig {
  title: string;
  emojis: string[];
  commits: string[];
}

/** Parsed conventional commit. */
interface ParsedCommit {
  hash: string;
  type: string;
  scope: string | null;
  description: string;
  breaking: boolean;
  body: string;
}

/** Changelog generation options. */
interface ChangelogOptions {
  fromTag?: string;
  toTag: string;
  outputPath: string;
  repoUrl: string;
}

const DEFAULT_OPTIONS: ChangelogOptions = {
  toTag: "HEAD",
  outputPath: "CHANGELOG.md",
  repoUrl: "https://github.com/OpSoll/noc-iq-contracts",
};

/** Conventional commit type to section mapping. */
const TYPE_SECTIONS: Record<string, string> = {
  feat: "Features",
  fix: "Bug Fixes",
  docs: "Documentation",
  style: "Styling",
  refactor: "Refactoring",
  perf: "Performance",
  test: "Tests",
  build: "Build",
  ci: "CI/CD",
  chore: "Chores",
  revert: "Reverts",
};

/** Parse a conventional commit message. */
function parseCommit(line: string): ParsedCommit | null {
  // Match: hash type(scope)!: description
  const match = line.match(
    /^([a-f0-9]+)\s+(\w+)(?:\(([^)]+)\))?\!?:\s+(.+)$/,
  );
  if (!match) return null;

  const [, hash, type, scope, description] = match;
  const breaking = line.includes("!:") || line.includes("BREAKING CHANGE:");

  return {
    hash,
    type,
    scope: scope || null,
    description,
    breaking,
    body: "",
  };
}

/** Get git log between tags. */
function getGitLog(from?: string, to: string = "HEAD"): string {
  const fromArg = from ? `${from}..${to}` : to;
  try {
    return execSync(`git log ${fromArg} --pretty=format:"%h %s" --no-merges`, {
      encoding: "utf8",
      maxBuffer: 10 * 1024 * 1024,
    });
  } catch {
    return "";
  }
}

/** Get the latest tag. */
function getLatestTag(): string | undefined {
  try {
    return execSync("git describe --tags --abbrev=0", { encoding: "utf8" }).trim();
  } catch {
    return undefined;
  }
}

/** Generate changelog content from parsed commits. */
function generateChangelog(commits: ParsedCommit[], options: ChangelogOptions): string {
  const sections: Record<string, string[]> = {};
  const breakingChanges: string[] = [];

  // Group commits by type
  for (const commit of commits) {
    if (commit.breaking) {
      breakingChanges.push(
        `- **${commit.scope ? `${commit.scope}: ` : ""}${commit.description}** (${commit.hash})`,
      );
    }

    const section = TYPE_SECTIONS[commit.type] || "Other Changes";
    if (!sections[section]) {
      sections[section] = [];
    }

    const scope = commit.scope ? `**${commit.scope}:** ` : "";
    sections[section].push(`- ${scope}${commit.description} (${commit.hash})`);
  }

  // Build changelog
  const lines: string[] = [];
  const date = new Date().toISOString().split("T")[0];
  lines.push(`## [Unreleased] - ${date}`);
  lines.push("");

  // Breaking changes first
  if (breakingChanges.length > 0) {
    lines.push("### ⚠ BREAKING CHANGES");
    lines.push("");
    lines.push(...breakingChanges);
    lines.push("");
  }

  // Sections in priority order
  const sectionOrder = [
    "Features",
    "Bug Fixes",
    "Performance",
    "Refactoring",
    "Documentation",
    "Tests",
    "Build",
    "CI/CD",
    "Chores",
    "Reverts",
    "Other Changes",
  ];

  for (const section of sectionOrder) {
    const entries = sections[section];
    if (!entries || entries.length === 0) continue;

    lines.push(`### ${section}`);
    lines.push("");
    lines.push(...entries);
    lines.push("");
  }

  // Footer with comparison link
  if (options.fromTag) {
    lines.push("---");
    lines.push("");
    lines.push(
      `**Full Changelog**: ${options.repoUrl}/compare/${options.fromTag}...${options.toTag}`,
    );
  }

  return lines.join("\n");
}

/** Prepend new content to existing CHANGELOG.md. */
function prependToExisting(newContent: string, outputPath: string): string {
  if (!existsSync(outputPath)) {
    return `# Changelog\n\n${newContent}\n`;
  }

  const existing = readFileSync(outputPath, "utf8");
  // Find where the first version section starts (after the header)
  const headerEnd = existing.indexOf("\n## ");
  if (headerEnd === -1) {
    return `# Changelog\n\n${newContent}\n`;
  }

  const header = existing.substring(0, headerEnd + 1);
  const rest = existing.substring(headerEnd + 1);
  return `${header}\n${newContent}\n${rest}`;
}

/** Main entry point. */
function main(): void {
  const args = process.argv.slice(2);
  const options = { ...DEFAULT_OPTIONS };

  // Parse CLI arguments
  for (let i = 0; i < args.length; i++) {
    switch (args[i]) {
      case "--from":
        options.fromTag = args[++i];
        break;
      case "--to":
        options.toTag = args[++i];
        break;
      case "--output":
        options.outputPath = args[++i];
        break;
      case "--repo":
        options.repoUrl = args[++i];
        break;
    }
  }

  // Auto-detect from tag if not provided
  if (!options.fromTag) {
    options.fromTag = getLatestTag();
  }

  console.log(`Generating changelog from ${options.fromTag || "beginning"} to ${options.toTag}...`);

  // Get and parse commits
  const log = getGitLog(options.fromTag, options.toTag);
  const commits = log
    .split("\n")
    .filter((line) => line.trim())
    .map(parseCommit)
    .filter((c): c is ParsedCommit => c !== null);

  console.log(`Found ${commits.length} conventional commits`);

  if (commits.length === 0) {
    console.log("No commits to include in changelog");
    return;
  }

  // Generate changelog
  const newContent = generateChangelog(commits, options);
  const fullContent = prependToExisting(newContent, options.outputPath);

  // Write output
  writeFileSync(options.outputPath, fullContent, "utf8");
  console.log(`Changelog written to ${options.outputPath}`);
}

main();
