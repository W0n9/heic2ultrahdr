#!/bin/bash
# CI / 本地通用的 Rust 链接环境变量，与 scripts/local-env.sh 保持一致的变量面。
# 用法: source scripts/ci-env.sh（CI 内 $GITHUB_WORKSPACE 已定位到仓库根）
root="${GITHUB_WORKSPACE:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
root=$(realpath "$root")
export PKG_CONFIG_PATH="$root/pkg-config/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
export PKG_CONFIG_LIBDIR="$root/pkg-config/lib"
export PKG_CONFIG_ALL_STATIC=true
export TURBOJPEG_STATIC=1
export TURBOJPEG_LIB_DIR="$root/pkg-config/lib"
export TURBOJPEG_INCLUDE_PATH="$root/pkg-config/include"
export FFMPEG_PKG_CONFIG_PATH="$root/pkg-config/lib/pkgconfig"
# Linux：静态 libheif 是 C++ 产物且 Requires.private 的 libde265 不会传递链接。
# macOS：clang 的 libstdc++ 别名不存在，要链 libc++；libde265 同样需要显式链接。
# Windows 上由 msvc 链接器自行解析，这些 flag 不适用。
os="${RUNNER_OS:-$(uname -s)}"
if [ "$os" = "Linux" ]; then
    export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-L native=$root/pkg-config/lib -C link-arg=-lstdc++ -C link-arg=-lde265"
elif [ "$os" = "macOS" ]; then
    export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-L native=$root/pkg-config/lib -C link-arg=-lc++ -C link-arg=-lde265"
fi
