import importlib.util
import os
from pathlib import Path
import shutil
import subprocess
import sys
root=Path.cwd()
spec=importlib.util.spec_from_file_location('prepare_mpv',root/'scripts/prepare-mpv.py')
module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
env=module.compiler_environment(); buildroot=root/'target/mpv-build'
env['MSYS_NO_PATHCONV']='1'
env['PATH']=os.pathsep.join([str(buildroot/'tools/nasm-2.16.03'),r'C:\Program Files\Git\usr\bin',r'C:\Program Files\Git\mingw64\bin',env['PATH']])
source=buildroot/'sources/x264-b35605ace3ddf7c1a5d67a2eb553f034aef41d55'
work=buildroot/'build/x264-source'
if not work.exists(): shutil.copytree(source,work)
def posix(path):
 s=Path(path).as_posix(); return '/'+s[0].lower()+s[2:]
prefix=posix(buildroot/'install/probe')
cmd=(f"cd '{posix(work)}' && export CC=clang-cl AR=llvm-lib RANLIB=llvm-ranlib "
     f"AS=nasm && ./configure --host=x86_64-w64-mingw32 --prefix='{prefix}' "
     "--enable-static --disable-shared --disable-cli --enable-pic --extra-cflags=/MD "
     "--disable-opencl --bit-depth=8 --chroma-format=420 && "
     "make -j6 && make install")
log=buildroot/'logs/x264-build.log'; log.parent.mkdir(parents=True,exist_ok=True)
with log.open('w',encoding='utf-8') as out:
 out.write('BASH: '+cmd+'\n'); out.flush()
 result=subprocess.run([r'C:\Program Files\Git\bin\bash.exe','-c',cmd],env=env,stdout=out,stderr=subprocess.STDOUT,text=True)
print(f'x264 configure/build exit: {result.returncode}; log: {log}')
if result.returncode: print('\n'.join(log.read_text(encoding='utf-8',errors='replace').splitlines()[-70:]))
sys.exit(result.returncode)


