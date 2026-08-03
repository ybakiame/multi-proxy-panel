#!/usr/bin/env bash
set -euo pipefail

# ProxyPanel Android 客户端内置 GEO 数据更新脚本
#
# 从 MetaCubeX/meta-rules-dat 的 latest release 下载 mihomo 启动所需的 GEO
# 数据三件套（geoip.metadb / geosite.dat / GeoLite2-ASN.mmdb），落盘到 Android
# 工程 assets/geo/ 目录（文件名按 mihomo v1.19.29 期望的 HomeDir 磁盘文件名
# 保存，详见下方「文件名映射」），并记录 release 的 tag/published_at 到
# assets/geo/VERSION。
#
# 背景:
#   mihomo 配置含 GEOIP/GEOSITE/ASN 规则时，启动需加载 GEO 数据文件；缺失时
#   mihomo 尝试从 GitHub 下载（首次无代理环境必败）导致启动失败。APK 内置
#   三件套后，MihomoVpnService 首启会把缺失文件复制到 HomeDir（= filesDir）。
#
# 文件名映射（以 mihomo v1.19.29 源码为准，constant/path.go）:
#   - `Path.MMDB()`  默认回退 `geoip.metadb`（asset 名与磁盘名一致）
#   - `Path.GeoSite()` 大小写不敏感（EqualFold）匹配 `GeoSite.dat`，
#     小写 `geosite.dat` 可被匹配，保持 asset 原名落盘
#   - `Path.ASN()`   仅匹配 `ASN.mmdb`：asset 名 `GeoLite2-ASN.mmdb`
#     不会被 EqualFold 匹配，必须落盘为 `ASN.mmdb`，否则 mihomo 找不到
#     文件会回退到 HomeDir 下载（无代理必败）
#
# 用法:
#   ./scripts/update-android-geodata.sh
#
# 环境要求:
#   - gh CLI（已登录 github.com，用于读取 latest release 元数据）
#   - curl（下载资产；不可用时自动回退 wget）
#
# 产物（不入库，见 app/.gitignore 的 /src/main/assets/geo/）:
#   crates/pp-client-ui/src-tauri/gen/android/app/src/main/assets/geo/
#     geoip.metadb  geosite.dat  ASN.mmdb  VERSION
#   GEO 数据文件总量约 25MB，构建 APK 前先运行本脚本生成 assets。

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

REPO="MetaCubeX/meta-rules-dat"
# latest 标签与 mihomo config.go 默认 GeoXUrl 下载地址一致
ASSETS_DIR="$PROJECT_ROOT/crates/pp-client-ui/src-tauri/gen/android/app/src/main/assets/geo"
VERSION_FILE="$ASSETS_DIR/VERSION"

# release 资产 -> mihomo HomeDir 磁盘文件名（asset:target）
ASSETS=(
  "geoip.metadb:geoip.metadb"
  "geosite.dat:geosite.dat"
  "GeoLite2-ASN.mmdb:ASN.mmdb"
)

command -v gh >/dev/null 2>&1 || { echo "error: 未找到 gh CLI（需已登录 github.com）" >&2; exit 1; }
command -v curl >/dev/null 2>&1 || command -v wget >/dev/null 2>&1 || { echo "error: 未找到 curl 或 wget" >&2; exit 1; }

# 读取 latest release 元数据
echo "==> 读取 $REPO latest release 元数据 ..."
TAG="$(gh release view --repo "$REPO" latest --json tagName --jq '.tagName')"
PUBLISHED_AT="$(gh release view --repo "$REPO" latest --json publishedAt --jq '.publishedAt')"
echo "==> tag=$TAG published_at=$PUBLISHED_AT"

mkdir -p "$ASSETS_DIR"

# 下载三个资产并重命名为 mihomo 期望的磁盘文件名
for entry in "${ASSETS[@]}"; do
  asset="${entry%%:*}"
  target="${entry##*:}"
  url="https://github.com/$REPO/releases/download/$TAG/$asset"
  echo "==> 下载 $asset -> $target ..."
  if command -v curl >/dev/null 2>&1; then
    curl -fL --retry 3 --retry-delay 2 -o "$ASSETS_DIR/$target" "$url"
  else
    wget -O "$ASSETS_DIR/$target" "$url"
  fi
  if [[ ! -s "$ASSETS_DIR/$target" ]]; then
    echo "error: $target 下载结果为空（0 字节），请检查网络或资产是否存在" >&2
    exit 1
  fi
done

# 记录 release 元数据（VERSION 文件）
printf 'tag: %s\npublished_at: %s\n' "$TAG" "$PUBLISHED_AT" > "$VERSION_FILE"

echo "==> 完成: $ASSETS_DIR"
for f in geoip.metadb geosite.dat ASN.mmdb VERSION; do
  echo "    $f: $(du -h "$ASSETS_DIR/$f" | cut -f1)"
done
echo "    GEO 数据文件为下载产物，不入库。"
