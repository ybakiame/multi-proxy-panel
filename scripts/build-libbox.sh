#!/usr/bin/env bash
set -euo pipefail

# ProxyPanel Android 客户端 libbox.aar 源码构建脚本
#
# 从 SagerNet/sing-box 源码构建 Android 版 libbox.aar（本地构建产物，不入库）。
#
# 用法:
#   ./scripts/build-libbox.sh [SING_BOX_VERSION] [OUTPUT_AAR]
#
# 环境要求:
#   - Go 工具链（>= 1.23，sing-box 模块要求；推荐 go1.24.x）
#   - JDK 17（javac/jar，gomobile 生成 Java 绑定）
#   - Android SDK + NDK（含 clang 交叉编译工具链）
#   - 可设置的环境变量:
#       GO_ROOT     Go SDK 根目录（默认自动从 PATH 探测，若不存在则用 ~/go-sdk）
#       GOPATH      Go module 缓存与工具安装目录（默认 ~/go-work）
#       JAVA_HOME   JDK 17 根目录
#       ANDROID_HOME        Android SDK 根目录（默认 ~/Android/Sdk）
#       ANDROID_NDK_HOME    NDK 根目录
#
# 许可约束（重要）:
#   sing-box 及其 libbox 构建产物遵循 GPL-3.0 许可。
#   生成的 libbox.aar 同样受 GPL-3.0 约束，分发应用前请确保满足
#   开源/源码可得性要求。详见 sing-box LICENSE。
#
# 构建说明:
#   - 使用 SagerNet 维护的 gomobile fork（v0.1.8，官方构建所用）而非上游
#     golang.org/x/mobile：上游新版要求模块内声明 tool 指令并升级 go 版本，
#     且以 go1.24/1.25 构建时出现 `invalid reference to os.checkPidfdOnce`
#     链接错误。
#   - 直接复用 sing-box 官方 `cmd/internal/build_libbox`，其内建
#     `-checklinkname=0`、正确 build tags、`-androidapi 21` 与
#     `-javapkg=io.nekohasekai`，保证与上游产出一致。

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

# 参数（可覆盖）
SING_BOX_VERSION="${1:-v1.12.9}"
OUTPUT_AAR="${2:-$PROJECT_ROOT/crates/pp-client-ui/src-tauri/gen/android/app/libs/libbox.aar}"

# 环境默认值
GOPATH="${GOPATH:-$HOME/go-work}"
GOMOBILE_TAG="v0.1.8"
WORK_DIR="${WORK_DIR:-/tmp/opencode/sing-box}"
ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"

# 探测 Go 工具链
if command -v go >/dev/null 2>&1; then
    GO_BIN="$(command -v go)"
else
    GO_ROOT="${GO_ROOT:-$HOME/go-sdk}"
    GO_BIN="$GO_ROOT/go/bin/go"
fi
if [[ ! -x "$GO_BIN" ]]; then
    echo "error: go 工具链未找到（可设置 GO_ROOT 或加入 PATH）" >&2
    exit 1
fi
export PATH="$(dirname "$GO_BIN"):$PATH"

# 检查环境变量
: "${JAVA_HOME:?请设置 JAVA_HOME（JDK 17）}"
: "${ANDROID_NDK_HOME:?请设置 ANDROID_NDK_HOME（NDK 路径）}"
[[ -d "$ANDROID_HOME" ]] || { echo "error: ANDROID_HOME=$ANDROID_HOME 不存在" >&2; exit 1; }
[[ -d "$ANDROID_NDK_HOME" ]] || { echo "error: ANDROID_NDK_HOME=$ANDROID_NDK_HOME 不存在" >&2; exit 1; }
command -v javac >/dev/null 2>&1 || { echo "error: javac 未在 PATH（需 JDK 17）" >&2; exit 1; }

export GOPATH
export ANDROID_HOME ANDROID_NDK_HOME
export PATH="$GOPATH/bin:$JAVA_HOME/bin:$PATH"

echo "==> sing-box 版本: $SING_BOX_VERSION"
echo "==> 输出: $OUTPUT_AAR"
echo "==> Go: $("$GO_BIN" version)"

# 1. 安装 SagerNet gomobile fork（含 gobind）
if ! command -v gomobile >/dev/null 2>&1 || ! command -v gobind >/dev/null 2>&1; then
    echo "==> 安装 sagernet/gomobile@$GOMOBILE_TAG ..."
    "$GO_BIN" install "github.com/sagernet/gomobile/cmd/gomobile@$GOMOBILE_TAG"
    "$GO_BIN" install "github.com/sagernet/gomobile/cmd/gobind@$GOMOBILE_TAG"
fi
command -v gomobile >/dev/null 2>&1 || { echo "error: gomobile 安装失败" >&2; exit 1; }
command -v gobind >/dev/null 2>&1 || { echo "error: gobind 安装失败" >&2; exit 1; }
gomobile init

# 2. 获取 sing-box 源码（浅克隆或复用）
if [[ ! -d "$WORK_DIR/.git" ]]; then
    echo "==> 克隆 sing-box@$SING_BOX_VERSION -> $WORK_DIR ..."
    git clone --depth 1 --branch "$SING_BOX_VERSION" https://github.com/SagerNet/sing-box.git "$WORK_DIR"
else
    echo "==> 复用 $WORK_DIR，切换到 $SING_BOX_VERSION ..."
    git -C "$WORK_DIR" fetch --depth 1 origin tag "$SING_BOX_VERSION"
    git -C "$WORK_DIR" checkout --detach "$SING_BOX_VERSION"
fi

# 3. 构建 AAR（官方 build_libbox：含 checklinkname=0、build tags、androidapi 21、javapkg）
echo "==> 构建 libbox.aar（多 ABI，耗时较长）..."
cd "$WORK_DIR"
go run ./cmd/internal/build_libbox -target android
if [[ ! -f "$WORK_DIR/libbox.aar" ]]; then
    echo "error: 构建完成但未找到 libbox.aar" >&2
    exit 1
fi

# 4. 落位
echo "==> 复制到 $OUTPUT_AAR ..."
mkdir -p "$(dirname "$OUTPUT_AAR")"
cp "$WORK_DIR/libbox.aar" "$OUTPUT_AAR"
echo "==> 完成: $(du -h "$OUTPUT_AAR" | cut -f1) ($(du -h "$WORK_DIR/libbox.aar" | cut -f1))"
echo "    libbox.aar 为 GPL-3.0 本地构建产物，不入库。"
