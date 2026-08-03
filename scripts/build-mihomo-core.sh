#!/usr/bin/env bash
set -euo pipefail

# ProxyPanel Android 客户端 mihomo.aar 源码构建脚本
#
# 将 android/mihomo-core（module: mihomocore）gomobile wrapper 构建为
# Android AAR（本地构建产物，不入库）。
#
# 用法:
#   ./scripts/build-mihomo-core.sh [OUTPUT_AAR]
#
# 环境要求:
#   - Go 工具链（>= 1.23，mihomo 模块要求；与 gomobile v0.1.8 兼容，
#     推荐 go1.24.x，见下方说明）
#   - JDK 17 或 21（javac/jar，gomobile 生成 Java 绑定）
#   - Android SDK + NDK（含 clang 交叉编译工具链）
#   - 可设置的环境变量:
#       GO_ROOT     Go SDK 根目录（默认自动从 PATH 探测，若不存在则用 ~/go-sdk）
#       GOPATH      Go module 缓存与工具安装目录（默认 ~/go-work）
#       JAVA_HOME   JDK 17 或 21 根目录
#       ANDROID_HOME        Android SDK 根目录（默认 ~/Android/Sdk）
#       ANDROID_NDK_HOME    NDK 根目录
#
# 构建说明:
#   - 使用 SagerNet 维护的 gomobile fork（v0.1.8，与 scripts/build-libbox.sh
#     一致）：上游 golang.org/x/mobile 以 go1.24/1.25/1.26 构建时会出现
#     `invalid reference to os.checkPidfdOnce` 链接错误。
#   - 必须带 `-tags cmfa`：mihomo 的 `dns.UpdateSystemDNS` /
#     `FlushCacheWithDefaultResolver` 仅在 `android && cmfa` build tag 下
#     编译（dns/patch_android.go），wrapper 的 UpdateDns 依赖它们。
#   - ABI 只编 arm64-v8a + x86_64，与
#     crates/pp-client-ui/src-tauri/gen/android/app/build.gradle.kts 的
#     abiFilters 对齐。

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# 参数（可覆盖）
OUTPUT_AAR="${1:-$PROJECT_ROOT/crates/pp-client-ui/src-tauri/gen/android/app/libs/mihomo.aar}"

# 环境默认值
GOPATH="${GOPATH:-$HOME/go-work}"
GOMOBILE_TAG="v0.1.8"
ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"

# 探测 Go 工具链（GO_ROOT 优先，其次 PATH，最后 ~/go-sdk）
if [[ -n "${GO_ROOT:-}" ]]; then
    if [[ -x "$GO_ROOT/bin/go" ]]; then
        GO_BIN="$GO_ROOT/bin/go"
    elif [[ -x "$GO_ROOT/go/bin/go" ]]; then
        GO_BIN="$GO_ROOT/go/bin/go"
    else
        echo "error: GO_ROOT=$GO_ROOT 下未找到 go 工具链" >&2
        exit 1
    fi
elif command -v go >/dev/null 2>&1; then
    GO_BIN="$(command -v go)"
else
    GO_BIN="$HOME/go-sdk/go/bin/go"
fi
if [[ ! -x "$GO_BIN" ]]; then
    echo "error: go 工具链未找到（可设置 GO_ROOT 或加入 PATH）" >&2
    exit 1
fi
export PATH="$(dirname "$GO_BIN"):$PATH"

# 检查环境变量
: "${JAVA_HOME:?请设置 JAVA_HOME（JDK 17 或 21）}"
: "${ANDROID_NDK_HOME:?请设置 ANDROID_NDK_HOME（NDK 路径）}"
[[ -d "$ANDROID_HOME" ]] || { echo "error: ANDROID_HOME=$ANDROID_HOME 不存在" >&2; exit 1; }
[[ -d "$ANDROID_NDK_HOME" ]] || { echo "error: ANDROID_NDK_HOME=$ANDROID_NDK_HOME 不存在" >&2; exit 1; }
command -v javac >/dev/null 2>&1 || { echo "error: javac 未在 PATH（需 JDK 17 或 21）" >&2; exit 1; }

# 校验 JDK 版本（17 或 21）
JAVA_MAJOR="$(javac -version 2>&1 | sed -E 's/^javac ([0-9]+).*/\1/')"
case "$JAVA_MAJOR" in
    17|21) ;;
    *)
        echo "error: JDK 版本不受支持（当前 $JAVA_MAJOR），需要 17 或 21" >&2
        exit 1
        ;;
esac

export GOPATH
export ANDROID_HOME ANDROID_NDK_HOME
export PATH="$GOPATH/bin:$JAVA_HOME/bin:$PATH"

echo "==> 输出: $OUTPUT_AAR"
echo "==> Go: $("$GO_BIN" version)"
echo "==> JDK: $(javac -version 2>&1)"

# 1. 安装 SagerNet gomobile fork（含 gobind）
if ! command -v gomobile >/dev/null 2>&1 || ! command -v gobind >/dev/null 2>&1; then
    echo "==> 安装 sagernet/gomobile@$GOMOBILE_TAG ..."
    "$GO_BIN" install "github.com/sagernet/gomobile/cmd/gomobile@$GOMOBILE_TAG"
    "$GO_BIN" install "github.com/sagernet/gomobile/cmd/gobind@$GOMOBILE_TAG"
fi
command -v gomobile >/dev/null 2>&1 || { echo "error: gomobile 安装失败" >&2; exit 1; }
command -v gobind >/dev/null 2>&1 || { echo "error: gobind 安装失败" >&2; exit 1; }
gomobile init

# 2. 构建 AAR（android/arm64 + android/amd64，-tags cmfa，androidapi 26）
echo "==> 构建 mihomo.aar（多 ABI，耗时较长）..."
mkdir -p "$(dirname "$OUTPUT_AAR")"
cd "$PROJECT_ROOT/android/mihomo-core"
gomobile bind -v -target android/arm64,android/amd64 -tags cmfa -androidapi 26 \
    -javapkg com.proxypanel.mihomocore \
    -o "$OUTPUT_AAR" \
    mihomocore

if [[ ! -f "$OUTPUT_AAR" ]]; then
    echo "error: 构建完成但未找到 $OUTPUT_AAR" >&2
    exit 1
fi

echo "==> 完成: $(du -h "$OUTPUT_AAR" | cut -f1)"
echo "    mihomo.aar 为 GPL-3.0 本地构建产物，不入库。"
