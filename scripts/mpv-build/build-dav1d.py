import importlib.util
import os
from pathlib import Path
import subprocess
import sys
root=Path.cwd()
spec=importlib.util.spec_from_file_location('prepare_mpv',root/'scripts/prepare-mpv.py')
module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
env=module.compiler_environment(); b=root/'target/mpv-build'
env['PATH']=os.pathsep.join([str(b/'tools/nasm-2.16.03'),str(b/'tools/pkgconf/bin'),r'C:\Program Files\Git\usr\bin',env['PATH']])
env['PKG_CONFIG']=str(b/'tools/pkgconf/bin/pkgconf.exe')
env['PKG_CONFIG_PATH']=os.pathsep.join([str(b/'install/probe/lib/pkgconfig'),str(root/'vcpkg_installed/x64-windows-v3-static-md-release/lib/pkgconfig')])
env['MESON_PACKAGE_CACHE_DIR']=str(b/'dependencies/subprojects/packagecache')
src=b/'sources/dav1d-b546257f770768b2c88258c533da38b91a06f737'; build=b/'build/dav1d'; prefix=(b/'install/probe').as_posix()
meson=[str(b/'tools/python/Scripts/python.exe'),'-m','mesonbuild.mesonmain']
steps=[('configure',meson+['setup',str(build),str(src),'--prefix='+prefix,'--buildtype=release','--default-library=static','-Db_vscrt=md','-Denable_tools=false','-Denable_examples=false','-Denable_tests=false','-Denable_docs=false','-Denable_asm=true']),('compile',meson+['compile','-C',str(build),'--jobs','6']),('install',meson+['install','-C',str(build)])]
log=b/'logs/dav1d-build.log'; log.parent.mkdir(parents=True,exist_ok=True)
with log.open('w',encoding='utf-8') as out:
 for name,args in steps:
  out.write('\n'+name.upper()+': '+subprocess.list2cmdline(args)+'\n');out.flush()
  result=subprocess.run(args,env=env,stdout=out,stderr=subprocess.STDOUT,text=True)
  if result.returncode:
   print(f'dav1d {name} exit: {result.returncode}; log: {log}')
   print('\n'.join(log.read_text(encoding='utf-8',errors='replace').splitlines()[-50:]))
   sys.exit(result.returncode)
print(f'dav1d static build/install succeeded; log: {log}')
