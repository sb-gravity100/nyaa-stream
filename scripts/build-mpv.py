"""Build the locked, app-targeted Windows libmpv candidate."""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PYTHON = sys.executable
HELPERS = ROOT / "scripts" / "mpv-build"

STAGES = [
    ("prepare-tools", [PYTHON, str(ROOT / "scripts" / "prepare-mpv.py")]),
    ("prepare-sources", [PYTHON, str(ROOT / "scripts" / "prepare-mpv.py"), "--sources-only"]),
    ("build-zlib", [PYTHON, str(HELPERS / "build-zlib.py")]),
    ("build-x264", [PYTHON, str(HELPERS / "build-x264.py")]),
    ("build-dav1d", [PYTHON, str(HELPERS / "build-dav1d.py")]),
    ("build-shaderc", [PYTHON, str(HELPERS / "build-shaderc.py")]),
    ("build-spirv-cross", [PYTHON, str(HELPERS / "build-spirv-cross.py")]),
    ("build-vulkan", [PYTHON, str(HELPERS / "build-vulkan.py")]),
    ("configure-ffmpeg", [PYTHON, str(HELPERS / "configure-ffmpeg.py")]),
    ("build-ffmpeg", [PYTHON, str(HELPERS / "build-ffmpeg.py")]),
    ("configure-mpv", [PYTHON, str(HELPERS / "configure-candidate.py")]),
    ("build-mpv", [PYTHON, str(HELPERS / "compile-candidate.py")]),
]


def run_stage(name: str, command: list[str], log) -> int:
    print(f"\n=== {name} ===", flush=True)
    log.write(f"\n=== {name} ===\nCOMMAND: {subprocess.list2cmdline(command)}\n")
    log.flush()
    process = subprocess.Popen(
        command,
        cwd=ROOT,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        encoding="utf-8",
        errors="replace",
        bufsize=1,
    )
    assert process.stdout is not None
    for line in process.stdout:
        print(line, end="", flush=True)
        log.write(line)
    code = process.wait()
    log.write(f"STAGE_EXIT: {code}\n")
    log.flush()
    if code:
        print(f"Stage {name} failed with exit code {code}; see target/mpv-build/logs/build-mpv.log", file=sys.stderr)
    return code


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.parse_args()
    if sys.platform != "win32":
        parser.error("the locked build recipe targets native Windows x64")
    build_root = ROOT / "target" / "mpv-build"
    logs = build_root / "logs"
    logs.mkdir(parents=True, exist_ok=True)
    with (logs / "build-mpv.log").open("w", encoding="utf-8", newline="\n") as log:
        for name, command in STAGES:
            code = run_stage(name, command, log)
            if code:
                return code
    dll = build_root / "build" / "candidate" / "mpv-2.dll"
    if not dll.is_file():
        print(f"Build completed but candidate DLL is missing: {dll}", file=sys.stderr)
        return 1
    print(f"Candidate ready: {dll} ({dll.stat().st_size:,} bytes)")
    print("The app bundle is not modified by this command.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
