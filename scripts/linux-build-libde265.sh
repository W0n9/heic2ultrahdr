#!/bin/bash
set -ex
root=$(realpath "${1:-/}")
path=$root/libde265
install=$(realpath "${2:-/opt/pkg-config}")
JOBS="${JOBS:-$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)}"
echo "Building libheif in $path"
if [ ! -d "$path" ]; then
    mkdir $path
    git clone --depth 1 --branch v1.0.15 https://github.com/strukturag/libde265.git $path
fi
build=$path/build
if [ ! -d "$build" ]; then
    mkdir $build
fi
cd $build
# CMake 4 移除了对 <3.5 的兼容，libde265 v1.0.15 还声明着 3.3.2；
# 这个变量是 CMake 官方的最低版本逃生口，旧 CMake 上无害。
cmake -DCMAKE_POLICY_VERSION_MINIMUM=3.5 \
    -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
    -DCMAKE_INSTALL_PREFIX=$install \
    $path
cmake --build $build --parallel "$JOBS"
