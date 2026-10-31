import importlib.util, os, subprocess, sys
from pathlib import Path
root=Path.cwd(); spec=importlib.util.spec_from_file_location('prepare_mpv',root/'scripts/prepare-mpv.py')
module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
env=module.compiler_environment(); b=root/'target/mpv-build'; env['MSYS_NO_PATHCONV']='1'; env['SHELL']=r'C:\Program Files\Git\usr\bin\sh.exe'
def posix(path):
 p=Path(path).as_posix(); return '/'+p[0].lower()+p[2:]
env['PATH']=os.pathsep.join([str(b/'tools/nasm-2.16.03'),str(b/'tools/pkgconf/bin'),r'C:\ProgramData\mingw64\mingw64\bin',r'C:\Program Files\Git\usr\bin',r'C:\Program Files\Git\mingw64\bin',env['PATH']])
env['PKG_CONFIG']=posix(b/'tools/pkgconf/bin/pkgconf.exe')
env['PKG_CONFIG_PATH']=os.pathsep.join([str(b/'install/probe/lib/pkgconfig'),str(root/'vcpkg_installed/x64-windows-v3-static-md-release/lib/pkgconfig')])
makefile=b/'build/ffmpeg-source/Makefile'
make_text=makefile.read_text(encoding='utf-8')
shell_line='SHELL := '+str(b/'tools/sh.exe').replace('\\','/')+'\n'
make_lines=[line for line in make_text.splitlines() if not line.startswith('SHELL :=')]
makefile.write_text(shell_line+'\n'.join(make_lines)+'\n',encoding='utf-8',newline='\n')
cmd='mingw32-make -j6 && mingw32-make install'
log=b/'logs/ffmpeg-build.log'; log.parent.mkdir(parents=True,exist_ok=True)
with log.open('w',encoding='utf-8') as out:
 out.write('BASH: cd build/ffmpeg && '+cmd+'\n');out.flush()
 result=subprocess.run([r'C:\Program Files\Git\bin\bash.exe','-c',f"cd '{posix(b/'build/ffmpeg-source')}' && {cmd}"],env=env,stdout=out,stderr=subprocess.STDOUT,text=True)
if result.returncode == 0:
 for pc in (b/'install/ffmpeg/lib/pkgconfig').glob('*.pc'):
  content=pc.read_text(encoding='utf-8')
  pc.write_text(content.replace('-libpath:', '/LIBPATH:'),encoding='utf-8',newline='\n')
print(f'FFmpeg build/install exit: {result.returncode}; log: {log}')
if result.returncode: print('\n'.join(log.read_text(encoding='utf-8',errors='replace').splitlines()[-60:]))
sys.exit(result.returncode)
