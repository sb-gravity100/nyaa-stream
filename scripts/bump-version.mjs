// Bumps the app version in every place it lives, keeping them in lockstep:
// package.json, src-tauri/tauri.conf.json, src-tauri/Cargo.toml.
//
//   npm run bump -- patch|minor|major|<x.y.z>
//
// Then commit and tag: git commit -am "chore: release vX.Y.Z" && git tag vX.Y.Z
import { readFileSync, writeFileSync } from "node:fs";

const files = {
  "package.json": /("version":\s*")([^"]+)(")/,
  "src-tauri/tauri.conf.json": /("version":\s*")([^"]+)(")/,
  // First `version =` in the file is the [package] one.
  "src-tauri/Cargo.toml": /(^version\s*=\s*")([^"]+)(")/m,
};

const arg = process.argv[2];
if (!arg) {
  console.error("usage: npm run bump -- patch|minor|major|<x.y.z>");
  process.exit(1);
}

const current = readFileSync("package.json", "utf8").match(files["package.json"])[2];
const [maj, min, pat] = current.split(".").map(Number);
const next =
  arg === "major" ? `${maj + 1}.0.0`
  : arg === "minor" ? `${maj}.${min + 1}.0`
  : arg === "patch" ? `${maj}.${min}.${pat + 1}`
  : arg;
if (!/^\d+\.\d+\.\d+$/.test(next)) {
  console.error(`invalid version: ${next}`);
  process.exit(1);
}

for (const [file, re] of Object.entries(files)) {
  const text = readFileSync(file, "utf8");
  if (!re.test(text)) {
    console.error(`no version field found in ${file}`);
    process.exit(1);
  }
  writeFileSync(file, text.replace(re, `$1${next}$3`));
  console.log(`${file}: ${current} -> ${next}`);
}
console.log(`\nNext: cargo check (refreshes Cargo.lock), then\n  git commit -am "chore: release v${next}" && git tag v${next}`);
