#!/usr/bin/env bash
set -euo pipefail

# ProxyPanel Android 客户端 panelcore.aar 源码构建脚本
#
# 将 android/panel-core（module: panelcore）一次 gomobile bind 同时导出
# sing-box libbox + mihomo wrapper，产出单一 panelcore.aar（本地构建产物，
# 不入库）。解决两个独立 AAR 内嵌相同 go.* 运行时类冲突无法共存的问题。
#
# 用法:
#   ./apps/android/scripts/build-panel-core.sh [OUTPUT_AAR]
#
# 环境要求:
#   - Go 工具链（go1.24.5，推荐 ~/go-sdk/go；系统 go1.26.x 与 sagernet
#     gomobile v0.1.8 不兼容，勿用）
#   - JDK 17 或 21（javac/jar，gomobile 生成 Java 绑定）
#   - Android SDK + NDK（含 clang 交叉编译工具链）
#   - 可设置的环境变量:
#       GO_ROOT     Go SDK 根目录（优先；其次 ~/go-sdk，最后 PATH）
#       GOPATH      Go module 缓存与工具安装目录（默认 ~/go-work）
#       JAVA_HOME   JDK 17 或 21 根目录
#       ANDROID_HOME        Android SDK 根目录（默认 ~/Android/Sdk）
#       ANDROID_NDK_HOME    NDK 根目录
#
# 构建说明:
#   - 使用 SagerNet 维护的 gomobile fork（v0.1.8，sing-box 官方构建所用）：
#     上游 golang.org/x/mobile 以 go1.24/1.25/1.26 构建时会出现
#     `invalid reference to os.checkPidfdOnce` 链接错误。
#   - bind 参数逐项对齐 sing-box 官方 cmd/internal/build_libbox（v1.12.9）：
#     -tags 并集 = mihomo `cmfa` + libbox release tags
#     （with_gvisor,with_quic,with_wireguard,with_utls,with_clash_api,
#     with_conntrack,with_tailscale）
#     -ldflags 含 `-checklinkname=0`（官方 release 构建同款）、
#     -X sing-box constant.Version、-s -w -buildid=
#     差异：-androidapi 26（对齐 App minSdk）、-javapkg com.proxypanel.core
#     （官方为 21 / io.nekohasekai）。
#   - ABI 只编 arm64-v8a + x86_64，与
#     apps/desktop/src-tauri/gen/android/app/build.gradle.kts 的
#     abiFilters 对齐。
#   - libbox 包不在本 module 内，由 mihomocore/gomobile.go 的 blank import
#     钉进 go.mod，gobind 的 packages.Load 方可解析。
#
# 许可约束（重要）:
#   sing-box 与 mihomo 均遵循 GPL-3.0 许可，合并构建产物 panelcore.aar
#   同样受 GPL-3.0 约束，分发应用前请确保满足开源/源码可得性要求。

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$(dirname "$SCRIPT_DIR")/../../.." && pwd)"

# 参数（可覆盖）
OUTPUT_AAR="${1:-$REPO_ROOT/apps/desktop/src-tauri/gen/android/app/libs/panelcore.aar}"

# 环境默认值
GOPATH="${GOPATH:-$HOME/go-work}"
GOMOBILE_TAG="v0.1.8"
ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
SING_BOX_VERSION="v1.12.9"
SING_BOX_VERSION_NUM="${SING_BOX_VERSION#v}"

# 探测 Go 工具链（GO_ROOT 优先，其次 ~/go-sdk，最后 PATH）
if [[ -n "${GO_ROOT:-}" ]]; then
    if [[ -x "$GO_ROOT/bin/go" ]]; then
        GO_BIN="$GO_ROOT/bin/go"
    elif [[ -x "$GO_ROOT/go/bin/go" ]]; then
        GO_BIN="$GO_ROOT/go/bin/go"
    else
        echo "error: GO_ROOT=$GO_ROOT 下未找到 go 工具链" >&2
        exit 1
    fi
elif [[ -x "$HOME/go-sdk/go/bin/go" ]]; then
    GO_BIN="$HOME/go-sdk/go/bin/go"
elif command -v go >/dev/null 2>&1; then
    GO_BIN="$(command -v go)"
    echo "warning: 使用 PATH 中的 go（$(dirname "$GO_BIN")），系统 go1.26.x 与 gomobile v0.1.8 不兼容，推荐 ~/go-sdk/go" >&2
else
    echo "error: go 工具链未找到（可设置 GO_ROOT 或安装 ~/go-sdk/go）" >&2
    exit 1
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

# 2. 构建 AAR（一次 bind 合并 libbox + mihomocore，参数对齐 build_libbox）
echo "==> 构建 panelcore.aar（libbox + mihomocore，多 ABI，耗时较长）..."
mkdir -p "$(dirname "$OUTPUT_AAR")"
cd "$REPO_ROOT/apps/android/panel-core"
gomobile bind -v -x \
    -target android/arm64,android/amd64 \
    -androidapi 26 \
    -javapkg com.proxypanel.core \
    -trimpath \
    -buildvcs=false \
    -ldflags "-X github.com/sagernet/sing-box/constant.Version=$SING_BOX_VERSION_NUM -s -w -buildid= -checklinkname=0" \
    -tags "cmfa,with_gvisor,with_quic,with_wireguard,with_utls,with_clash_api,with_conntrack,with_tailscale" \
    -o "$OUTPUT_AAR" \
    panelcore/mihomocore github.com/sagernet/sing-box/experimental/libbox

if [[ ! -f "$OUTPUT_AAR" ]]; then
    echo "error: 构建完成但未找到 $OUTPUT_AAR" >&2
    exit 1
fi

echo "==> 完成: $(du -h "$OUTPUT_AAR" | cut -f1)"
echo "    panelcore.aar 为 GPL-3.0 本地构建产物，不入库。"
