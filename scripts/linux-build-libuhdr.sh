#!/bin/bash
set -ex
root=$(realpath "${1:-/}")
path=$root/libuhdr
install=$(realpath "${2:-/opt/pkg-config}")
JOBS="${JOBS:-$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)}"
echo "Building libuhdr in $path"
if [ ! -d "$path" ]; then
    mkdir $path
    git clone --depth 1 --branch v1.4.0 https://github.com/google/libultrahdr.git $path
fi
build=$path/build
if [ ! -d "$build" ]; then
    mkdir $build
fi
cd $build
# 显式指向自建的静态 libjpeg（stage A 先构建），不依赖 runner 镜像预装的
# libjpeg-dev——ubuntu-26.04 镜像不再预装它，导致 CMake 找不到 JPEG 而失败。
# gcc 15 的新 libstdc++ 不再传递包含 <cstdint>，libuhdr v1.4.0 头文件裸用
# uint8_t 会编译失败，强制包含补上。
cmake -DCMAKE_BUILD_TYPE=Release -DUHDR_BUILD_EXAMPLES=OFF -DCMAKE_INSTALL_PREFIX=$install \
     -DBUILD_SHARED_LIBS=OFF -DUHDR_ENABLE_INSTALL=ON \
     -DJPEG_LIBRARY=$install/lib/libjpeg.a -DJPEG_INCLUDE_DIR=$install/include \
     -DCMAKE_CXX_FLAGS="-include cstdint" \
    $path
cmake --build $build --parallel "$JOBS"
