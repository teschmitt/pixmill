#!/usr/bin/env node
// Bumps the project version in package.json and Cargo.toml (workspace.package.version),
// then refreshes Cargo.lock. tauri.conf.json reads its version from package.json
// via the "version": "../package.json" indirection, so it does not need updating.
//
// Usage:
//   node scripts/bump-version.mjs <major|minor|patch>
//
// Stdout: the new version (e.g. "0.2.0"). Other diagnostics go to stderr so
// callers can capture the version with `NEW=$(node scripts/bump-version.mjs minor)`.

import { readFileSync, writeFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, "..");

const bump = process.argv[2];
if (!["major", "minor", "patch"].includes(bump)) {
  console.error(`Usage: bump-version.mjs <major|minor|patch>`);
  process.exit(2);
}

const pkgPath = resolve(repoRoot, "package.json");
const cargoPath = resolve(repoRoot, "Cargo.toml");

const pkg = JSON.parse(readFileSync(pkgPath, "utf8"));
const current = pkg.version;
const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(current);
if (!match) {
  console.error(`package.json version "${current}" is not plain semver MAJOR.MINOR.PATCH`);
  process.exit(1);
}

let [major, minor, patch] = match.slice(1).map(Number);
if (bump === "major") {
  major += 1;
  minor = 0;
  patch = 0;
} else if (bump === "minor") {
  minor += 1;
  patch = 0;
} else {
  patch += 1;
}
const next = `${major}.${minor}.${patch}`;
console.error(`Bumping ${current} → ${next} (${bump})`);

pkg.version = next;
writeFileSync(pkgPath, JSON.stringify(pkg, null, "\t") + "\n");

// Cargo.toml: rewrite only the workspace.package.version line. Avoids pulling
// in a TOML parser dep; the regex is anchored to the section to skip any
// other "version" keys.
const cargoSrc = readFileSync(cargoPath, "utf8");
const replaced = cargoSrc.replace(
  /(\[workspace\.package\][\s\S]*?\nversion\s*=\s*")[^"]+(")/,
  `$1${next}$2`
);
if (replaced === cargoSrc) {
  console.error("Failed to update workspace.package.version in Cargo.toml");
  process.exit(1);
}
writeFileSync(cargoPath, replaced);

// Refresh Cargo.lock so the workspace crates pick up the new version.
// `cargo update --workspace` re-resolves only workspace members; if it's not
// available on older toolchains, fall back to a no-op `cargo metadata`.
try {
  execFileSync("cargo", ["update", "--workspace"], { cwd: repoRoot, stdio: "inherit" });
} catch (err) {
  console.error("cargo update --workspace failed:", err.message);
  process.exit(1);
}

// New version on stdout — the workflow captures this.
process.stdout.write(next + "\n");
