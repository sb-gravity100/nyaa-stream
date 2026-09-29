// Downloads the prebuilt native libs (static libtorrent, OpenSSL, FFmpeg) into
// vcpkg_installed/, so building needs no vcpkg. Run once per clone:
//
//   npm run setup
//
// The archive is the release asset `native-deps-v1` (built once from
// vcpkg.json; see PLAN.md). Bump VERSION/SHA256 together when it is rebuilt.
import { createHash } from "node:crypto";
import { createReadStream, createWriteStream, existsSync, mkdirSync, rmSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

const VERSION = "native-deps-v1";
const SHA256 = "a7b4f8dee206415d4742bcd26b4037ce566ceda00e709567a285d733a1b2eedd";
const URL = `https://github.com/sb-gravity100/nyaa-stream/releases/download/${VERSION}/${VERSION}.zip`;
const TRIPLET_DIR = "vcpkg_installed/x64-windows-v3-static-md-release";

if (existsSync(TRIPLET_DIR) && !process.argv.includes("--force")) {
  console.log(`${TRIPLET_DIR} already present (use --force to re-download)`);
  process.exit(0);
}

const zip = join(tmpdir(), `${VERSION}.zip`);
console.log(`downloading ${URL} (~1.3 GB)`);
const res = await fetch(URL);
if (!res.ok || !res.body) {
  console.error(`download failed: HTTP ${res.status}`);
  process.exit(1);
}
await pipeline(Readable.fromWeb(res.body), createWriteStream(zip));

const hash = createHash("sha256");
await pipeline(createReadStream(zip), hash);
if (hash.digest("hex") !== SHA256) {
  console.error("checksum mismatch - refusing to extract");
  rmSync(zip, { force: true });
  process.exit(1);
}

mkdirSync(".", { recursive: true });
console.log("extracting");
// Windows 10+ ships bsdtar, which reads zip.
execFileSync("tar", ["-xf", zip], { stdio: "inherit" });
rmSync(zip, { force: true });
console.log("done - native libs are in vcpkg_installed/");
