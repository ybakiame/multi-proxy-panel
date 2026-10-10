#!/usr/bin/env bash
# android-ndk-env.sh — CI runner 的 Android 交叉编译工具链环境变量。
#
# nix dev shell 已由 flake.nix 导出同一组变量（nix store NDK），本脚本只服务于
# CI runner；本地开发统一进入仓库 nix develop：从 ANDROID_NDK_HOME / NDK_HOME /
# $ANDROID_HOME/ndk/<最新版本> 推导 NDK 位置，导出 cc-rs（CC/AR）、最终链接
# （CARGO_TARGET_*_LINKER）与 rquickjs-sys bindgen（BINDGEN_EXTRA_CLANG_ARGS_*）
# 所需的变量。mobile 仅发布 arm64，故只导出 aarch64 一套。
#
# 用法（必须 source，让变量进入当前 shell）：
#   CI runner: source apps/client/scripts/android-ndk-env.sh
#   本地开发: nix develop

# 1) 定位 NDK
_NDK="${ANDROID_NDK_HOME:-${NDK_HOME:-}}"
if [[ -z "$_NDK" ]]; then
    _SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"
    if [[ -n "$_SDK" && -d "$_SDK/ndk" ]]; then
        # 取版本号最大的已安装 NDK
        _NDK="$(ls -d "$_SDK"/ndk/*/ 2>/dev/null | sort -V | tail -1)"
        _NDK="${_NDK%/}"
    fi
fi
if [[ -z "$_NDK" || ! -d "$_NDK" ]]; then
    echo "android-ndk-env: 未找到 NDK，请先设置 ANDROID_NDK_HOME / NDK_HOME / ANDROID_HOME" >&2
    return 1 2>/dev/null || exit 1
fi

# 2) 定位 NDK 内 LLVM 工具链（仅支持 linux-x86_64 / darwin 主机）
case "$(uname -s)-$(uname -m)" in
    Linux-x86_64)  _PREBUILT=linux-x86_64 ;;
    Darwin-arm64)  _PREBUILT=darwin-arm64 ;;
    Darwin-x86_64) _PREBUILT=darwin-x86_64 ;;
    *) echo "android-ndk-env: 不支持的主机平台 $(uname -s)-$(uname -m)" >&2
       return 1 2>/dev/null || exit 1 ;;
esac
_LLVM="$_NDK/toolchains/llvm/prebuilt/$_PREBUILT"
if [[ ! -d "$_LLVM" ]]; then
    echo "android-ndk-env: NDK LLVM 工具链不存在：$_LLVM" >&2
    return 1 2>/dev/null || exit 1
fi

# 3) 导出（minSdk 33 → *-android33-clang；minSdk 变更时同步调整）
_API=33
_CLANG="$_LLVM/bin/aarch64-linux-android${_API}-clang"
export CC_aarch64_linux_android="$_CLANG"
export AR_aarch64_linux_android="$_LLVM/bin/llvm-ar"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$_CLANG"
export BINDGEN_EXTRA_CLANG_ARGS_aarch64_linux_android="--target=aarch64-linux-android${_API} --sysroot=$_LLVM/sysroot -isystem $_LLVM/sysroot/usr/include"
# tauri CLI 探测 NDK 优先读 NDK_HOME
export NDK_HOME="$_NDK"

echo "android-ndk-env: NDK=$_NDK (api=$_API, prebuilt=$_PREBUILT)"
