import importlib.util
import os
import subprocess
import sys
from pathlib import Path


root = Path.cwd()
spec = importlib.util.spec_from_file_location("prepare_mpv", root / "scripts/prepare-mpv.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
env = module.compiler_environment()
buildroot = root / "target/mpv-build"
source = buildroot / "dependencies/subprojects/zlib-1.3.2"
build = buildroot / "build/zlib"
prefix = buildroot / "install/probe"
env["PATH"] = os.pathsep.join([r"C:\Program Files\Git\usr\bin", env["PATH"]])

if not (source / "zlib.h").is_file():
    raise SystemExit(f"locked zlib source is missing: {source}")

args = [
    "cmake", "-S", str(source), "-B", str(build), "-G", "Ninja",
    "-DCMAKE_C_COMPILER=clang-cl",
    "-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreadedDLL",
    "-DCMAKE_BUILD_TYPE=Release",
    "-DCMAKE_INSTALL_PREFIX=" + str(prefix),
    "-DZLIB_BUILD_SHARED=OFF", "-DZLIB_BUILD_STATIC=ON",
    "-DZLIB_BUILD_TESTING=OFF", "-DZLIB_INSTALL=ON",
]
log = buildroot / "logs/zlib-build.log"
log.parent.mkdir(parents=True, exist_ok=True)
with log.open("w", encoding="utf-8") as out:
    out.write("CONFIGURE: " + subprocess.list2cmdline(args) + "\n")
    out.flush()
    result = subprocess.run(args, env=env, stdout=out, stderr=subprocess.STDOUT, text=True)
    if result.returncode == 0:
        build_args = ["cmake", "--build", str(build), "--parallel", "6"]
        out.write("\nBUILD: " + subprocess.list2cmdline(build_args) + "\n")
        out.flush()
        result = subprocess.run(build_args, env=env, stdout=out, stderr=subprocess.STDOUT, text=True)
    if result.returncode == 0:
        install_args = ["cmake", "--install", str(build)]
        out.write("\nINSTALL: " + subprocess.list2cmdline(install_args) + "\n")
        out.flush()
        result = subprocess.run(install_args, env=env, stdout=out, stderr=subprocess.STDOUT, text=True)

if result.returncode:
    print(f"zlib build failed with exit code {result.returncode}; log: {log}")
    print("\n".join(log.read_text(encoding="utf-8", errors="replace").splitlines()[-45:]))
    raise SystemExit(result.returncode)

pkgconfig = prefix / "lib/pkgconfig"
pkgconfig.mkdir(parents=True, exist_ok=True)
pc = "\n".join([
    "prefix=" + prefix.as_posix(),
    "exec_prefix=${prefix}",
    "libdir=${exec_prefix}/lib",
    "includedir=${prefix}/include",
    "",
    "Name: zlib",
    "Description: zlib compression library",
    "Version: 1.3.2",
    "Libs: -L${libdir} -lzs",
    "Cflags: -I${includedir}",
    "",
])
(pkgconfig / "zlib.pc").write_text(pc, encoding="utf-8", newline="\n")
if not (prefix / "include/zlib.h").is_file() or not (prefix / "lib/zs.lib").is_file():
    raise SystemExit("zlib installation is missing its static library or headers")
print(f"Pinned zlib 1.3.2 static library installed for FFmpeg; log: {log}")
