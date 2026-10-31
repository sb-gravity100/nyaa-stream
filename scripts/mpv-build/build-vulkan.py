import importlib.util
import os
from pathlib import Path
import subprocess
import sys

root=Path.cwd()
spec=importlib.util.spec_from_file_location('prepare_mpv',root/'scripts/prepare-mpv.py')
module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
env=module.compiler_environment(); env['PATH']=os.pathsep.join([r'C:\Program Files\Git\usr\bin',env['PATH']])
buildroot=root/'target/mpv-build'; prefix=buildroot/'install/probe'
headers=buildroot/'dependencies/subprojects/Vulkan-Headers-1.4.346'
loader=buildroot/'sources/Vulkan-Loader-6923f2b7e869dd94a09dcdd6fa2152269edc7fef'
steps=[
 ('headers configure',['cmake','-S',str(headers),'-B',str(buildroot/'build/vulkan-headers'),'-G','Ninja','-DCMAKE_C_COMPILER=clang','-DCMAKE_CXX_COMPILER=clang++','-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreadedDLL','-DCMAKE_BUILD_TYPE=Release','-DCMAKE_INSTALL_PREFIX='+str(prefix),' -DVULKAN_HEADERS_ENABLE_TESTS=OFF']),
]
# Normalize the argument added above without introducing shell parsing.
steps[0][1][-1]=steps[0][1][-1].strip()
steps += [
 ('headers install',['cmake','--build',str(buildroot/'build/vulkan-headers'),'--target','install','--parallel','6']),
 ('loader configure',['cmake','-S',str(loader),'-B',str(buildroot/'build/vulkan-loader'),'-G','Ninja','-DCMAKE_C_COMPILER=clang','-DCMAKE_CXX_COMPILER=clang++','-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreadedDLL','-DCMAKE_BUILD_TYPE=Release','-DCMAKE_INSTALL_PREFIX='+str(prefix),'-DVULKAN_HEADERS_INSTALL_DIR='+str(prefix),'-DBUILD_TESTS=OFF','-DUPDATE_DEPS=OFF']),
 ('loader install',['cmake','--build',str(buildroot/'build/vulkan-loader'),'--target','install','--parallel','6']),
]
log=buildroot/'logs/vulkan-loader-build.log'; log.parent.mkdir(parents=True,exist_ok=True)
with log.open('w',encoding='utf-8') as out:
 for label,args in steps:
  out.write('\n'+label.upper()+': '+subprocess.list2cmdline(args)+'\n'); out.flush()
  result=subprocess.run(args,env=env,stdout=out,stderr=subprocess.STDOUT,text=True)
  if result.returncode:
   print(f'{label} exit: {result.returncode}; log: {log}')
   print('\n'.join(log.read_text(encoding='utf-8',errors='replace').splitlines()[-55:]))
   sys.exit(result.returncode)
print(f'Vulkan headers and loader installed; log: {log}')
