"""Prepare isolated native libmpv build tools; does not build or install libmpv."""

import argparse
import configparser
import hashlib
import json
import logging
import os
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import sys
import urllib.request
import venv
import zipfile


LOG = logging.getLogger("prepare-mpv")
REPO = Path(__file__).resolve().parents[1]
ROOT = REPO / "target" / "mpv-build"


def run(args, env=None, capture=False):
    args = [str(arg) for arg in args]
    # CreateProcess resolves the executable using the parent environment;
    # resolve tools explicitly when PATH exists only in the child environment.
    executable = shutil.which(args[0], path=(env or os.environ).get("PATH"))
    if not executable:
        raise RuntimeError(f"Command executable not found: {args[0]}")
    args[0] = executable
    LOG.info("Running %s", subprocess.list2cmdline(args))
    if capture:
        result = subprocess.run(args, check=True, env=env, stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, text=True)
    else:
        output_path = ROOT / "logs/tool-commands.log"
        with output_path.open("a", encoding="utf-8") as output:
            output.write("\nCOMMAND: " + subprocess.list2cmdline(args) + "\n")
            output.flush()
            result = subprocess.run(args, env=env, stdout=output, stderr=subprocess.STDOUT, text=True)
        if result.returncode:
            LOG.error("Command failed; diagnostics: %s", output_path)
            raise subprocess.CalledProcessError(result.returncode, args)
    return result.stdout.strip() if capture else ""


def digest(path):
    LOG.debug("Hashing %s", path)
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def fetch(entry):
    LOG.info("Preparing locked archive %s", entry["file"])
    archive = ROOT / "downloads" / entry["file"]
    archive.parent.mkdir(parents=True, exist_ok=True)
    if not archive.exists():
        LOG.info("Downloading public upstream archive %s", entry["url"])
        request = urllib.request.Request(entry["url"], headers={"User-Agent": "nyaa-stream-mpv-build"})
        partial = archive.with_name(archive.name + ".part")
        with urllib.request.urlopen(request, timeout=60) as response, partial.open("wb") as output:
            shutil.copyfileobj(response, output)
        if digest(partial) != entry["sha256"]:
            raise RuntimeError(f"Checksum mismatch for {entry['file']}; partial archive retained")
        partial.replace(archive)
    if digest(archive) != entry["sha256"]:
        raise RuntimeError(f"Checksum mismatch for cached {entry['file']}; archive retained")
    LOG.info("Archive checksum verified: %s", archive.name)
    return archive


def unpack(archive, destination, directory):
    LOG.info("Preparing archive directory %s", directory)
    destination.mkdir(parents=True, exist_ok=True)
    marker = destination / (directory + ".archive-sha256")
    archive_hash = digest(archive)
    if marker.exists():
        if marker.read_text(encoding="utf-8").strip() != archive_hash:
            raise RuntimeError(f"Existing extraction belongs to another archive: {directory}")
        LOG.info("Reusing extraction %s", directory)
        return destination / directory
    if (destination / directory).exists():
        raise RuntimeError(f"Unmarked extraction exists; preserving {destination / directory}")
    with zipfile.ZipFile(archive) as bundle:
        for member in bundle.infolist():
            relative = PurePosixPath(member.filename)
            if (relative.is_absolute() or ".." in relative.parts or ":" in member.filename
                    or "\\" in member.filename or not relative.parts or relative.parts[0] != directory
                    or (member.external_attr >> 16) & 0o170000 == 0o120000):
                raise RuntimeError(f"Unsupported ZIP entry in {archive.name}")
        bundle.extractall(destination)
    marker.write_text(archive_hash + "\n", encoding="utf-8")
    LOG.info("Archive extracted: %s", directory)
    return destination / directory


def compiler_environment():
    LOG.info("Locating native Windows compiler environment")
    vswhere = Path(os.environ.get("ProgramFiles(x86)", r"C:\Program Files (x86)")) / "Microsoft Visual Studio/Installer/vswhere.exe"
    installation = run([vswhere, "-latest", "-products", "*", "-requires",
                        "Microsoft.VisualStudio.Component.VC.Tools.x86.x64", "-property",
                        "installationPath"], capture=True)
    if not installation:
        raise RuntimeError("Visual Studio x64 build tools not found")
    activation = Path(installation) / "VC/Auxiliary/Build/vcvars64.bat"
    LOG.info("Importing Visual Studio environment; environment values are not logged")
    # Capture privately: the shell environment can contain unrelated credentials.
    command_shell = subprocess.list2cmdline([os.environ.get("COMSPEC", "cmd.exe")])
    result = subprocess.run(
        f'{command_shell} /d /s /c ""{activation}" >nul && set"',
        check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
    )
    # Windows keys are case-insensitive; avoid duplicate Path/PATH entries in
    # the child block when importing cmd's differently cased environment.
    environment = {key.upper(): value for key, value in os.environ.items()}
    for line in result.stdout.splitlines():
        key, separator, value = line.partition("=")
        if separator and key:
            environment[key.upper()] = value
    llvm = Path(os.environ.get("ProgramFiles", r"C:\Program Files")) / "LLVM/bin"
    for tool in ("clang.exe", "clang++.exe", "lld-link.exe", "llvm-lib.exe", "llvm-rc.exe"):
        if not (llvm / tool).is_file():
            raise RuntimeError(f"Required LLVM tool missing: {tool}")
    environment["PATH"] = str(llvm) + os.pathsep + environment["PATH"]
    environment.update(CC="clang", CXX="clang++", CC_LD="lld-link", CXX_LD="lld-link",
                       CFLAGS="-march=x86-64", CXXFLAGS="-march=x86-64", LDFLAGS="")
    LOG.info("Native x64 compiler environment prepared")
    return environment


def fetch_wrap_sources(wraps):
    LOG.info("Preparing %s pinned Meson source definitions", len(wraps))
    workspace = ROOT / "dependencies"
    subprojects = workspace / "subprojects"
    subprojects.mkdir(parents=True, exist_ok=True)
    project = workspace / "meson.build"
    project_text = "project('mpv-source-lock', 'c')\n"
    if project.exists() and project.read_text(encoding="utf-8-sig").strip() != project_text.strip():
        raise RuntimeError("Existing source metadata project differs; preserving it")
    project.write_text(project_text, encoding="utf-8", newline="\n")
    directories = {}
    for name, entry in wraps.items():
        LOG.info("Verifying locked source definition: %s", name)
        payload = (REPO / entry["file"]).read_bytes().replace(b"\r\n", b"\n")
        if hashlib.sha256(payload).hexdigest() != entry["sha256_lf"]:
            raise RuntimeError(f"Source definition checksum mismatch: {name}")
        destination = subprojects / (name + ".wrap")
        if destination.exists() and destination.read_bytes().replace(b"\r\n", b"\n") != payload:
            raise RuntimeError(f"Existing source definition differs; preserving {name}")
        destination.write_bytes(payload)
        directories[name] = str(subprojects / entry["directory"])
    python = ROOT / "tools/python/Scripts/python.exe"
    # Windows Meson directory locking failed with concurrent source extraction;
    # use one worker for repeatable download/extraction, without changing hashes.
    environment = os.environ.copy()
    environment["MESON_PACKAGE_CACHE_DIR"] = str(subprojects / "packagecache")
    run([python, "-m", "mesonbuild.mesonmain", "subprojects", "download",
         "--sourcedir", workspace, "-j", "1", *wraps.keys()], env=environment)
    for name, entry in wraps.items():
        LOG.info("Verifying cached source/build-patch archives: %s", name)
        definition = configparser.ConfigParser(interpolation=None)
        definition.read(subprojects / (name + ".wrap"), encoding="utf-8")
        section = definition["wrap-file"]
        for kind in ("source", "patch"):
            if kind + "_filename" not in section:
                continue
            archive = subprojects / "packagecache" / section[kind + "_filename"]
            if not archive.is_file() or digest(archive) != section[kind + "_hash"]:
                raise RuntimeError(f"Cached {kind} archive checksum mismatch: {name}")
        if not Path(directories[name]).is_dir():
            raise RuntimeError(f"Source directory was not prepared: {name}")
    LOG.info("Pinned Meson sources prepared")
    return directories


def fetch_sources():
    LOG.info("Starting locked source preparation; compilation is disabled in this mode")
    lock_path = REPO / "native/mpv/sources.lock.json"
    lock = json.loads(lock_path.read_text(encoding="utf-8"))
    if lock["schema"] != 1:
        raise RuntimeError("Unsupported source lock schema")
    directories = {}
    for name, entry in lock["packages"].items():
        LOG.info("Preparing source %s at %s", name, entry["commit"])
        archive = fetch(entry)
        directories[name] = str(unpack(archive, ROOT / "sources", entry["directory"]))
    if lock.get("wraps"):
        LOG.info("Preparing transitive source archives through pinned Meson definitions")
        directories.update(fetch_wrap_sources(lock["wraps"]))
    for patch in lock.get("patches", []):
        LOG.info("Verifying locked native build patch: %s", patch["file"])
        payload = (REPO / patch["file"]).read_bytes().replace(b"\r\n", b"\n")
        if hashlib.sha256(payload).hexdigest() != patch["sha256_lf"]:
            raise RuntimeError("Native build patch checksum mismatch")
    manifest = {"source_lock_sha256": digest(lock_path), "directories": directories,
                "transitive_lock_complete": lock["transitive_lock_complete"],
                "pending": lock["pending"]}
    (ROOT / "sources-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    if lock["transitive_lock_complete"]:
        LOG.info("Locked sources prepared; no compilation performed")
    else:
        LOG.warning("Core sources prepared; %s transitive lock items still pending", len(lock["pending"]))


def main(sources_only=False):
    LOG.info("Starting isolated libmpv preparation; sources_only=%s", sources_only)
    if os.name != "nt":
        raise RuntimeError("Native Windows tool setup requires Windows")
    if sources_only:
        LOG.info("Selecting source-only mode")
        fetch_sources()
        return
    lock = json.loads((REPO / "native/mpv/tools.lock.json").read_text(encoding="utf-8"))
    if lock["schema"] != 1:
        raise RuntimeError("Unsupported tools lock schema")
    archives = {name: fetch(lock[name]) for name in ("meson", "nasm", "pkgconf")}
    tools = ROOT / "tools"
    python = tools / "python/Scripts/python.exe"
    if not python.exists():
        LOG.info("Creating local Python virtual environment")
        venv.EnvBuilder(with_pip=True).create(tools / "python")
    run([python, "-m", "pip", "install", "--disable-pip-version-check", "--no-index",
         "--no-deps", archives["meson"]])
    nasm_dir = unpack(archives["nasm"], tools, lock["nasm"]["directory"])
    pkgconf_source = unpack(archives["pkgconf"], ROOT / "sources", lock["pkgconf"]["directory"])
    environment = compiler_environment()
    environment["PATH"] = str(nasm_dir) + os.pathsep + str(python.parent) + os.pathsep + environment["PATH"]
    meson = [python, "-m", "mesonbuild.mesonmain"]
    build = ROOT / "build/pkgconf"
    prefix = tools / "pkgconf"
    setup = meson + ["setup", build, pkgconf_source, "--prefix=" + str(prefix),
                     "--buildtype=release", "--default-library=static", "-Db_vscrt=md",
                     "-Dtests=disabled", "--wrap-mode=nodownload"]
    if (build / "meson-private/coredata.dat").exists():
        LOG.info("Reconfiguring existing pkgconf build")
        setup.append("--reconfigure")
    run(setup, env=environment)
    run(meson + ["compile", "-C", build], env=environment)
    run(meson + ["install", "-C", build], env=environment)
    pkgconf = prefix / "bin/pkgconf.exe"
    versions = {
        "meson": run(meson + ["--version"], capture=True),
        "nasm": run([nasm_dir / "nasm.exe", "-v"], capture=True),
        "pkgconf": run([pkgconf, "--version"], capture=True),
        "clang": run(["clang", "--version"], env=environment, capture=True),
        "ninja": run(["ninja", "--version"], env=environment, capture=True),
    }
    for name in ("meson", "nasm", "pkgconf"):
        if lock[name]["version"] not in versions[name]:
            raise RuntimeError(f"Prepared tool version does not match lock: {name}")
    manifest = {
        "tools_lock_sha256": digest(REPO / "native/mpv/tools.lock.json"),
        "versions": versions,
        "executables": {"python": str(python), "nasm": str(nasm_dir / "nasm.exe"),
                        "pkgconf": str(pkgconf), "clang": shutil.which("clang", path=environment["PATH"])},
        "windows_sdk_version": environment.get("WINDOWSSDKVERSION"),
        "visual_studio_version": environment.get("VISUALSTUDIOVERSION"),
    }
    (ROOT / "tools-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    LOG.info("Tool preparation completed; manifest: %s", ROOT / "tools-manifest.json")
    LOG.info("No libmpv or media dependencies compiled; installed player unchanged")


if __name__ == "__main__":
    (ROOT / "logs").mkdir(parents=True, exist_ok=True)
    logging.basicConfig(level=logging.DEBUG, format="%(asctime)s %(levelname)s %(message)s",
                        handlers=[logging.StreamHandler(), logging.FileHandler(ROOT / "logs/prepare-tools.log", encoding="utf-8")])
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sources-only", action="store_true", help="fetch locked source archives without compiling")
    arguments = parser.parse_args()
    try:
        main(sources_only=arguments.sources_only)
    except Exception:
        LOG.exception("Tool preparation failed")
        sys.exit(1)
