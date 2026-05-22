#!/usr/bin/env node
// Prepends a new release section to CHANGELOG.md. Used by the release workflow
// after generating notes via the GitHub API.
//
// Usage:
//   node scripts/update-changelog.mjs <version> <body>
//
// The body is whatever GitHub's release-notes generator produced. We strip the
// auto-generated "## What's Changed" / "**Full Changelog**: ..." outer scaffold
// because Keep-a-Changelog style uses its own headings.

import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(__dirname, "..");
const changelogPath = resolve(repoRoot, "CHANGELOG.md");

const version = process.argv[2];
const rawBody = process.argv[3] ?? "";

if (!version) {
  console.error("Usage: update-changelog.mjs <version> <body>");
  process.exit(2);
}

const today = new Date().toISOString().slice(0, 10);

// GitHub wraps the categorized PR list in a "## What's Changed" heading and
// appends a "**Full Changelog**: <compare-url>" footer. Strip the outer
// heading; keep the footer (it's a useful compare link). The category
// headings inside (### Features, ### Fixes, etc.) survive.
const stripped = rawBody.replace(/^##\s+What's Changed\s*\n/m, "").trim();

const newSection = `## [${version}] - ${today}\n\n${stripped}\n`;

const header = `# Changelog

All notable changes to Pixmill are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/), and this project adheres to
[Semantic Versioning](https://semver.org/).

`;

let next;
if (!existsSync(changelogPath)) {
  next = header + newSection + "\n";
} else {
  const existing = readFileSync(changelogPath, "utf8");
  // Find the first "## " heading and splice the new section in front of it.
  // If no prior release exists, append after the header preamble.
  const firstReleaseIdx = existing.search(/^## \[/m);
  if (firstReleaseIdx === -1) {
    next = existing.replace(/\s*$/, "") + "\n\n" + newSection + "\n";
  } else {
    next = existing.slice(0, firstReleaseIdx) + newSection + "\n" + existing.slice(firstReleaseIdx);
  }
}

writeFileSync(changelogPath, next);
console.error(`Updated CHANGELOG.md with [${version}] - ${today}`);
