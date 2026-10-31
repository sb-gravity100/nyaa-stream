import importlib.util
import os
from pathlib import Path
import subprocess
import sys

root = Path.cwd()
spec = importlib.util.spec_from_file_location('prepare_mpv', root / 'scripts/prepare-mpv.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
env = module.compiler_environment()
mpv_build = root / 'target/mpv-build'
source = mpv_build / 'sources/shaderc-b64fd42c4a2a9d501c47aca5d3571f7673a5e9cc'
build = mpv_build / 'build/shaderc'
prefix = mpv_build / 'install/probe'
env['PATH'] = os.pathsep.join([r'C:\Program Files\Git\usr\bin', env['PATH']])
args = [
    'cmake', '-S', str(source), '-B', str(build), '-G', 'Ninja',
    '-DCMAKE_C_COMPILER=clang', '-DCMAKE_CXX_COMPILER=clang++',
    '-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreadedDLL', '-DCMAKE_BUILD_TYPE=Release',
    '-DCMAKE_INSTALL_PREFIX=' + str(prefix), '-DCMAKE_INSTALL_LIBDIR=lib',
    '-DSHADERC_ENABLE_WGSL_OUTPUT=OFF', '-DSHADERC_SKIP_TESTS=ON',
    '-DSHADERC_SKIP_EXAMPLES=ON', '-DSHADERC_SKIP_EXECUTABLES=ON',
    '-DSHADERC_SKIP_COPYRIGHT_CHECK=ON', '-DSHADERC_ENABLE_WERROR_COMPILE=OFF',
    '-DSPIRV_SKIP_EXECUTABLES=ON', '-DSPIRV_SKIP_TESTS=ON', '-DSPIRV_WERROR=OFF',
    '-DGLSLANG_TESTS=OFF', '-DENABLE_GLSLANG_BINARIES=OFF',
]
log = mpv_build / 'logs/shaderc-build.log'
log.parent.mkdir(parents=True, exist_ok=True)
with log.open('w', encoding='utf-8') as out:
    out.write('CONFIGURE: ' + subprocess.list2cmdline(args) + '\n')
    out.flush()
    result = subprocess.run(args, env=env, stdout=out, stderr=subprocess.STDOUT, text=True)
    if result.returncode == 0:
        build_args = ['cmake', '--build', str(build), '--parallel', '6']
        out.write('\nBUILD: ' + subprocess.list2cmdline(build_args) + '\n'); out.flush()
        result = subprocess.run(build_args, env=env, stdout=out, stderr=subprocess.STDOUT, text=True)
    if result.returncode == 0:
        install_args = ['cmake', '--install', str(build)]
        out.write('\nINSTALL: ' + subprocess.list2cmdline(install_args) + '\n'); out.flush()
        result = subprocess.run(install_args, env=env, stdout=out, stderr=subprocess.STDOUT, text=True)
print(f'shaderc build exit: {result.returncode}; log: {log}')
if result.returncode:
    print('\n'.join(log.read_text(encoding='utf-8', errors='replace').splitlines()[-55:]))
sys.exit(result.returncode)
