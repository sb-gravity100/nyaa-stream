// Builds, signs and publishes a release from this PC - what
// .github/workflows/release.yml did before GitHub Actions became unavailable.
//
//   npm run release -- --notes "<release notes>" [--dry-run] [--skip-build]
//
// Expects HEAD to be the `chore: release vX.Y.Z` commit tagged vX.Y.Z (npm run
// bump, commit, git tag). --dry-run builds and stages the files without
// pushing or publishing; --skip-build reuses the existing bundle output.
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const REPO = "sb-gravity100/nyaa-stream";
const BUNDLE = "target/release/bundle";
const STAGE = "target/release/publish";
const KEY_PATH = join(homedir(), ".tauri", "nyaa-stream.key");

const log = (msg, extra) => console.log(`[release] ${msg}${extra ? " " + JSON.stringify(extra) : ""}`);
const fail = (msg) => {
  console.error(`[release] error: ${msg}`);
  process.exit(1);
};

function parseArgs(argv) {
  const args = { notes: null, dryRun: false, skipBuild: false };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--dry-run") args.dryRun = true;
    else if (a === "--skip-build") args.skipBuild = true;
    else if (a === "--notes") args.notes = argv[++i] ?? null;
    else fail(`unknown argument: ${a}`);
  }
  if (!args.notes) fail('usage: npm run release -- --notes "<text>" [--dry-run] [--skip-build]');
  return args;
}

/** Runs a command; returns trimmed stdout, or exits on failure unless `allowFail`. */
function run(cmd, cmdArgs, { env, allowFail = false, inherit = false } = {}) {
  log("run", { cmd: [cmd, ...cmdArgs].join(" ") });
  const res = spawnSync(cmd, cmdArgs, {
    encoding: "utf8",
    shell: process.platform === "win32",
    env: env ?? process.env,
    stdio: inherit ? "inherit" : ["ignore", "pipe", "pipe"],
  });
  if (res.status !== 0 && !allowFail) {
    if (!inherit) console.error(res.stderr);
    fail(`command failed (${res.status}): ${cmd} ${cmdArgs[0] ?? ""}`);
  }
  return { ok: res.status === 0, out: (res.stdout ?? "").trim() };
}

function versions() {
  const pkg = JSON.parse(readFileSync("package.json", "utf8")).version;
  const conf = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8")).version;
  const cargo = readFileSync("src-tauri/Cargo.toml", "utf8").match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  return { pkg, conf, cargo };
}

function preflight(args) {
  log("preflight");
  const v = versions();
  if (v.pkg !== v.conf || v.pkg !== v.cargo) fail(`versions out of lockstep: ${JSON.stringify(v)}`);
  const version = v.pkg;
  const tag = `v${version}`;

  if (run("git", ["status", "--porcelain"]).out) fail("working tree is not clean");
  const headTags = run("git", ["tag", "--points-at", "HEAD"]).out.split(/\s+/);
  if (!headTags.includes(tag)) {
    const msg = `HEAD is not tagged ${tag} (tags at HEAD: ${headTags.filter(Boolean).join(", ") || "none"})`;
    // A dry run publishes nothing, so it can test the build before tagging.
    if (args.dryRun) log(`warning: ${msg} - fine for a dry run, required to publish`);
    else fail(msg);
  }
  if (!run("gh", ["auth", "status"], { allowFail: true }).ok) fail("gh is not logged in (gh auth login)");
  if (!existsSync("vcpkg_installed")) fail("vcpkg_installed/ missing - run npm run setup");
  if (!existsSync("src-tauri/lib/libmpv-2.dll")) fail("src-tauri/lib/libmpv-2.dll missing - see src-tauri/lib/README.md");
  if (!args.skipBuild && !existsSync(KEY_PATH)) fail(`signing key not found at ${KEY_PATH}`);
  if (!args.dryRun && run("gh", ["release", "view", tag, "--repo", REPO], { allowFail: true }).ok) {
    fail(`release ${tag} is already published`);
  }
  log("preflight ok", { version, tag });
  return { version, tag };
}

function build() {
  log("building (npm run tauri build)");
  // Tauri reads the key *contents*; the _PATH variant isn't honoured. Never logged.
  const env = {
    ...process.env,
    TAURI_SIGNING_PRIVATE_KEY: readFileSync(KEY_PATH, "utf8").trim(),
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD: "",
    // Dev builds link with rust-lld (.cargo/config.toml); shipped builds stay
    // on MSVC link.exe.
    CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER: "link.exe",
  };
  run("npm", ["run", "tauri", "build"], { env, inherit: true });
  log("build finished");
}

function stage({ version, tag }, notes) {
  const files = {
    setup: `${BUNDLE}/nsis/nyaa-stream_${version}_x64-setup.exe`,
    msi: `${BUNDLE}/msi/nyaa-stream_${version}_x64_en-US.msi`,
  };
  const assets = [];
  for (const path of Object.values(files)) {
    for (const f of [path, `${path}.sig`]) {
      if (!existsSync(f)) fail(`missing build output: ${f}`);
      assets.push(f);
    }
  }
  const setupName = files.setup.split("/").pop();
  const latest = {
    version,
    notes,
    pub_date: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
    platforms: {
      "windows-x86_64": {
        signature: readFileSync(`${files.setup}.sig`, "utf8").trim(),
        url: `https://github.com/${REPO}/releases/download/${tag}/${setupName}`,
      },
    },
  };
  mkdirSync(STAGE, { recursive: true });
  const latestPath = `${STAGE}/latest.json`;
  writeFileSync(latestPath, JSON.stringify(latest, null, 2) + "\n");
  assets.push(latestPath);
  log("staged", { assets });
  return assets;
}

function publish({ tag }, notes, assets) {
  log("pushing main and the release tag", { tag });
  run("git", ["push", "origin", "HEAD:main"], { inherit: true });
  // Only this tag - milestone tags stay local (PLAN.md "Release plan").
  run("git", ["push", "origin", `refs/tags/${tag}`], { inherit: true });
  log("creating GitHub release", { tag });
  run("gh", ["release", "create", tag, "--repo", REPO, "--verify-tag", "--title", `nyaa-stream ${tag}`, "--notes", notes, ...assets], { inherit: true });
  log("published", { url: `https://github.com/${REPO}/releases/tag/${tag}` });
}

const args = parseArgs(process.argv.slice(2));
log("start", { dryRun: args.dryRun, skipBuild: args.skipBuild });
const release = preflight(args);
if (args.skipBuild) log("skipping build, reusing bundle output");
else build();
const assets = stage(release, args.notes);
if (args.dryRun) log("dry run: not pushing or publishing");
else publish(release, args.notes, assets);
