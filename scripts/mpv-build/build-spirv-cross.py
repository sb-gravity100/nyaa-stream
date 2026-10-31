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
source = mpv_build / 'sources/SPIRV-Cross-a0fba56c34a6700f1724bf9b751da5b488a3775c'
build = mpv_build / 'build/spirv-cross'
prefix = mpv_build / 'install/probe'
env['PATH'] = os.pathsep.join([r'C:\Program Files\Git\usr\bin', env['PATH']])
args = [
    'cmake', '-S', str(source), '-B', str(build), '-G', 'Ninja',
    '-DCMAKE_C_COMPILER=clang', '-DCMAKE_CXX_COMPILER=clang++',
    '-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreadedDLL', '-DCMAKE_BUILD_TYPE=Release',
    '-DCMAKE_INSTALL_PREFIX=' + str(prefix), '-DCMAKE_INSTALL_LIBDIR=lib',
    '-DSPIRV_CROSS_STATIC=ON', '-DSPIRV_CROSS_SHARED=OFF',
    '-DSPIRV_CROSS_CLI=OFF', '-DSPIRV_CROSS_ENABLE_TESTS=OFF',
    '-DSPIRV_CROSS_ENABLE_GLSL=ON', '-DSPIRV_CROSS_ENABLE_HLSL=ON',
    '-DSPIRV_CROSS_ENABLE_MSL=OFF', '-DSPIRV_CROSS_ENABLE_CPP=OFF',
    '-DSPIRV_CROSS_ENABLE_REFLECT=OFF', '-DSPIRV_CROSS_ENABLE_UTIL=OFF',
]
log = mpv_build / 'logs/spirv-cross-build.log'
log.parent.mkdir(parents=True, exist_ok=True)
with log.open('w', encoding='utf-8') as out:
    out.write('CONFIGURE: ' + subprocess.list2cmdline(args) + '\n'); out.flush()
    result = subprocess.run(args, env=env, stdout=out, stderr=subprocess.STDOUT, text=True)
    if result.returncode == 0:
        build_args = ['cmake', '--build', str(build), '--parallel', '6']
        out.write('\nBUILD: ' + subprocess.list2cmdline(build_args) + '\n'); out.flush()
        result = subprocess.run(build_args, env=env, stdout=out, stderr=subprocess.STDOUT, text=True)
    if result.returncode == 0:
        install_args = ['cmake', '--install', str(build)]
        out.write('\nINSTALL: ' + subprocess.list2cmdline(install_args) + '\n'); out.flush()
        result = subprocess.run(install_args, env=env, stdout=out, stderr=subprocess.STDOUT, text=True)
print(f'SPIRV-Cross build exit: {result.returncode}; log: {log}')
if result.returncode:
    print('\n'.join(log.read_text(encoding='utf-8', errors='replace').splitlines()[-45:]))
    sys.exit(result.returncode)

# The C API exposes HLSL methods when HLSL is enabled but upstream's generated
# shared C API pkg-config file omits the static HLSL backend from its link interface.
pc = prefix / 'lib/pkgconfig/spirv-cross-c-shared.pc'
content = pc.read_text(encoding='utf-8')
expected = 'Libs: -L${libdir} -lspirv-cross-c -lspirv-cross-glsl -lspirv-cross-core'
if expected in content:
    content = content.replace(
        expected,
        'Libs: -L${libdir} -lspirv-cross-c -lspirv-cross-hlsl -lspirv-cross-glsl -lspirv-cross-core',
    )
elif '-lspirv-cross-hlsl' not in content:
    raise SystemExit(f'Unexpected SPIRV-Cross C API link line in {pc}')
pc.write_text(content, encoding='utf-8', newline='\n')
if not (prefix / 'lib/spirv-cross-hlsl.lib').is_file():
    raise SystemExit('SPIRV-Cross HLSL backend library is missing')
print('SPIRV-Cross C API metadata includes its HLSL backend library')
