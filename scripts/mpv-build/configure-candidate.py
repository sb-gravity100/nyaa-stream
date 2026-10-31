import importlib.util
import os
from pathlib import Path
import shutil
import subprocess
import sys

root = Path.cwd()
spec = importlib.util.spec_from_file_location('prepare_mpv', root / 'scripts/prepare-mpv.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
env = module.compiler_environment()
env['RC'] = r'C:\Program Files\LLVM\bin\llvm-rc.exe'
env['CC'] = 'clang-cl'
env['CXX'] = 'clang-cl'
mpv_build = root / 'target/mpv-build'
pkgconf = mpv_build / 'tools/pkgconf/bin/pkgconf.exe'
nasm = mpv_build / 'tools/nasm-2.16.03'
env['PATH'] = os.pathsep.join([str(pkgconf.parent), str(nasm), r'C:\\Program Files\\Git\\usr\\bin', env['PATH']])
env['PKG_CONFIG'] = str(pkgconf)
env['PKG_CONFIG_PATH'] = os.pathsep.join([str(mpv_build / 'install/ffmpeg/lib/pkgconfig'), str(mpv_build / 'install/probe/lib/pkgconfig')])
env['MESON_PACKAGE_CACHE_DIR'] = str(mpv_build / 'dependencies/subprojects/packagecache')
source = mpv_build / 'sources/mpv-41f6a645068483470267271e1d09966ca3b9f413'
libplacebo_source = mpv_build / 'sources/libplacebo-275f7dea27924cdd34ab2e6c20e448b6bc4fab65'
libplacebo_subproject = source / 'subprojects/libplacebo'
build = mpv_build / 'build/candidate'
if not (libplacebo_source / 'meson.build').is_file():
    raise SystemExit(f'locked patched libplacebo source is missing: {libplacebo_source}')
if not libplacebo_subproject.is_dir():
    raise SystemExit(f'mpv libplacebo subproject directory is missing: {libplacebo_subproject}')
# mpv's Meson subproject is a separate extraction; use the independently
# locked and patched source package that prepare-mpv.py verified.
shutil.copytree(libplacebo_source, libplacebo_subproject, dirs_exist_ok=True)
print(f'Synced locked patched libplacebo into the mpv subproject: {libplacebo_subproject}')
args = [
    str(mpv_build / 'tools/python/Scripts/python.exe'), '-m', 'mesonbuild.mesonmain',
    'setup', '--wipe', str(build), str(source), '--prefix=' + (mpv_build / 'install/candidate').as_posix(),
    '--buildtype=release', '--default-library=shared', '--wrap-mode=default',
    '-Db_vscrt=md', '-Dcplayer=false', '-Dlibmpv=true', '-Dgpl=true', '-Dbuild-date=false',
    '-Dtests=false', '-Dlua=luajit', '-Djavascript=disabled', '-Dcplugins=disabled',
    '-Dcdda=disabled', '-Ddvdnav=disabled', '-Dlibbluray=disabled', '-Ddvbin=disabled',
    '-Dlibavdevice=disabled', '-Dvapoursynth=disabled', '-Dsdl2-gamepad=disabled',
    '-Dlibarchive=disabled', '-Dwasapi=enabled', '-Dwin32-threads=enabled',
    '-Dd3d11=enabled', '-Ddirect3d=enabled', '-Dgl=enabled', '-Dgl-win32=enabled',
    '-Dvulkan=enabled', '-Dshaderc=enabled', '-Dspirv-cross=enabled',
    '-Djpeg=enabled', '-Dlcms2=enabled', '-Diconv=enabled', '-Dzlib=enabled', '-Duchardet=enabled',
    '-Dlibass:default_library=static', '-Dlibplacebo:default_library=static',
    '-Dfreetype2:default_library=static', '-Dfribidi:default_library=static',
    '-Dharfbuzz:default_library=static', '-Dharfbuzz:cpp_std=c++20', '-Dlcms2:default_library=static',
    '-Dlibjpeg-turbo:default_library=static', '-Dlibpng:default_library=static',
    '-Dlibunibreak:default_library=static', '-Dluajit:default_library=static',
    '-Duchardet:default_library=static', '-Dwin-iconv:default_library=static',
    '-Dzlib:default_library=static', '-Dbzip2:default_library=static',
    '-Dgraphite2:default_library=static',
]
log = mpv_build / 'logs/candidate-configure.log'
log.parent.mkdir(parents=True, exist_ok=True)
with log.open('w', encoding='utf-8') as out:
    out.write('COMMAND: ' + subprocess.list2cmdline(args) + '\n')
    out.flush()
    result = subprocess.run(args, env=env, stdout=out, stderr=subprocess.STDOUT, text=True)
print(f'Meson configure exit: {result.returncode}; log: {log}')
if result.returncode:
    print('\n'.join(log.read_text(encoding='utf-8', errors='replace').splitlines()[-65:]))
sys.exit(result.returncode)







