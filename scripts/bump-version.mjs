#!/usr/bin/env node
// Bumps the app version everywhere it is declared.
//
//   node scripts/bump-version.mjs          # 0.1.0 -> 0.1.1 (patch)
//   node scripts/bump-version.mjs 0.2.0    # set an explicit version
//
// Edits are plain text replacements so each file keeps its formatting and
// line endings (package.json / tauri.conf.json use CRLF, the rest LF).
// Prints the new version on stdout.
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (p) => readFileSync(join(root, p), "utf8");

function patchBump() {
  const current = JSON.parse(read("src-tauri/tauri.conf.json")).version;
  const m = /^(\d+)\.(\d+)\.(\d+)$/.exec(current);
  if (!m) throw new Error(`current version "${current}" is not major.minor.patch`);
  return `${m[1]}.${m[2]}.${Number(m[3]) + 1}`;
}

const next = process.argv[2] ?? patchBump();
if (!/^\d+\.\d+\.\d+$/.test(next)) throw new Error(`"${next}" is not major.minor.patch`);

const edits = [
  // First top-level "version" key
  ["package.json", /^(\s*"version":\s*")[^"]+(")/m],
  ["src-tauri/tauri.conf.json", /^(\s*"version":\s*")[^"]+(")/m],
  // Root "version" and packages[""].version
  ["package-lock.json", /^(\s*"name": "halo",\r?\n\s*"version": ")[^"]+(")/gm],
  ["src-tauri/Cargo.toml", /^(\[package\]\r?\nname = "halo"\r?\nversion = ")[^"]+(")/m],
  ["Cargo.lock", /^(name = "halo"\r?\nversion = ")[^"]+(")/m],
];

for (const [file, re] of edits) {
  const text = read(file);
  if (!re.test(text)) throw new Error(`version not found in ${file}`);
  re.lastIndex = 0;
  writeFileSync(join(root, file), text.replace(re, `$1${next}$2`));
}

console.log(next);
