import importlib.util
import os
from pathlib import Path
import subprocess
import sys
root=Path.cwd()
spec=importlib.util.spec_from_file_location('prepare_mpv',root/'scripts/prepare-mpv.py')
module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
env=module.compiler_environment(); env['RC']=r'C:\Program Files\LLVM\bin\llvm-rc.exe'; buildroot=root/'target/mpv-build'
env['PATH']=os.pathsep.join([str(buildroot/'tools/pkgconf/bin'),str(buildroot/'tools/nasm-2.16.03'),r'C:\Program Files\Git\usr\bin',env['PATH']])
env['PKG_CONFIG']=str(buildroot/'tools/pkgconf/bin/pkgconf.exe')
env['PKG_CONFIG_PATH']=os.pathsep.join([str(buildroot/'install/ffmpeg/lib/pkgconfig'),str(buildroot/'install/probe/lib/pkgconfig')])
env['MESON_PACKAGE_CACHE_DIR']=str(buildroot/'dependencies/subprojects/packagecache')
args=[str(buildroot/'tools/python/Scripts/python.exe'),'-m','mesonbuild.mesonmain','compile','-C',str(buildroot/'build/candidate'),'--jobs','6','mpv']
log=buildroot/'logs/candidate-compile.log'; log.parent.mkdir(parents=True,exist_ok=True)
with log.open('w',encoding='utf-8') as out:
 out.write('COMMAND: '+subprocess.list2cmdline(args)+'\n'); out.flush()
 result=subprocess.run(args,env=env,stdout=out,stderr=subprocess.STDOUT,text=True)
print(f'Meson compile exit: {result.returncode}; log: {log}')
if result.returncode: print('\n'.join(log.read_text(encoding='utf-8',errors='replace').splitlines()[-65:]))
sys.exit(result.returncode)


