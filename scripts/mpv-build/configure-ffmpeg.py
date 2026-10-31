import importlib.util
import os
from pathlib import Path
import subprocess
import sys
root=Path.cwd()
spec=importlib.util.spec_from_file_location('prepare_mpv',root/'scripts/prepare-mpv.py')
module=importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
env=module.compiler_environment(); b=root/'target/mpv-build'
env['MSYS_NO_PATHCONV']='1'
env['response_files']='yes'
def posix(path):
 s=Path(path).as_posix(); return '/'+s[0].lower()+s[2:]
pkg=b/'tools/pkgconf/bin/pkgconf.exe'; prefix=(b/'install/probe').as_posix()
vcpkg=(root/'vcpkg_installed/x64-windows-v3-static-md-release').resolve()
nv_headers=b/'sources/nv-codec-headers-43d91706e097565f57b311e567f0219838bcc2f6/include'
if not (nv_headers/'ffnvcodec/nvEncodeAPI.h').is_file():
 raise SystemExit(f'locked NVIDIA codec headers are missing: {nv_headers}')
nv_pc=b/'install/probe/lib/pkgconfig/ffnvcodec.pc'
nv_pc.parent.mkdir(parents=True,exist_ok=True)
nv_pc.write_text(
 'Name: ffnvcodec\nDescription: Locked FFmpeg NVIDIA codec headers\n'
 'Version: 11.1.5.3\nLibs:\nCflags: -I'+nv_headers.as_posix()+'\n',
 encoding='utf-8',newline='\n')
env['PATH']=os.pathsep.join([str(pkg.parent),str(b/'tools/nasm-2.16.03'),r'C:\Program Files\Git\usr\bin',r'C:\Program Files\Git\mingw64\bin',env['PATH']])
env['PKG_CONFIG']=posix(pkg)
env['PKG_CONFIG_PATH']=os.pathsep.join([str(b/'install/probe/lib/pkgconfig'),str(vcpkg/'lib/pkgconfig')])
env['INCLUDE']=os.pathsep.join([str(vcpkg/'include'),env.get('INCLUDE','')])
env['LIB']=os.pathsep.join([str(vcpkg/'lib'),env.get('LIB','')])
env['CC']='clang-cl'; env['CXX']='clang-cl'; env['AR']='llvm-lib'; env['RANLIB']='llvm-ranlib'; env['AS']='nasm'; env['RC']=r'C:\Program Files\LLVM\bin\llvm-rc.exe'
src=b/'sources/FFmpeg-8ae0b34901ba60a802f183ee75a250a9fc3e09a5'
build=b/'build/ffmpeg-source'
if not build.exists():
 import shutil; shutil.copytree(src,build)
flags=[
 '--prefix='+posix(b/'install/ffmpeg'),'--arch=x86_64','--target-os=win64','--toolchain=msvc',
 '--cc=clang-cl','--cxx=clang-cl','--ar=llvm-lib','--ranlib=llvm-ranlib','--as=nasm','--pkg-config='+env['PKG_CONFIG'],
 '--enable-static','--disable-shared','--enable-pic','--extra-cflags=/MD','--disable-programs','--disable-doc','--disable-debug','--disable-autodetect','--disable-everything','--disable-avdevice','--enable-gpl','--enable-network','--enable-w32threads','--enable-zlib','--enable-schannel',
 '--enable-avcodec','--enable-avformat','--enable-avfilter','--enable-avutil','--enable-swresample','--enable-swscale',
 '--enable-demuxers','--enable-parsers','--enable-bsfs',
 '--enable-protocol=file,http,https,tcp,tls,crypto,httpproxy,udp',
 '--enable-decoder=h264,hevc,av1,libdav1d,aac,ac3,eac3,flac,opus,ass,ssa,subrip,srt,webvtt,mov_text,dvbsub,dvdsub,hdmv_pgs_subtitle,png,mjpeg',
 '--enable-encoder=aac,libx264,h264_nvenc,h264_qsv,h264_amf,png,mjpeg',
 '--enable-muxer=mp4,mov,image2',
 '--enable-filter=scale,format,fps,setpts,asetpts,aresample,aformat,null,anull',
 '--enable-libx264','--enable-libdav1d','--enable-ffnvcodec','--enable-libmfx','--enable-amf','--enable-nvenc','--enable-d3d11va','--enable-dxva2',
 '--enable-hwaccel=av1_d3d11va,av1_d3d11va2,h264_d3d11va,h264_d3d11va2,hevc_d3d11va,hevc_d3d11va2',
]
args=['./configure',*flags]
cmd="cd '"+posix(build)+"' && "+subprocess.list2cmdline(args)
log=b/'logs/ffmpeg-configure.log'; log.parent.mkdir(parents=True,exist_ok=True)
with log.open('w',encoding='utf-8') as out:
 out.write('BASH: '+cmd+'\n');out.flush()
 result=subprocess.run([r'C:\Program Files\Git\bin\bash.exe','-c',cmd],env=env,stdout=out,stderr=subprocess.STDOUT,text=True)
print(f'FFmpeg configure exit: {result.returncode}; log: {log}')
if result.returncode: print('\n'.join(log.read_text(encoding='utf-8',errors='replace').splitlines()[-75:]))
sys.exit(result.returncode)





