#!/usr/bin/env bash
# ProxyPanel Hub bootstrap script
# Downloads the CLI binary then delegates to `proxy-panel install hub`.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/<repo>/main/scripts/install-hub.sh | bash -s -- [--version latest] [--repo owner/repo]
#   bash install-hub.sh --version v0.3.3
#
# Options:
#   --version <ver>      Release version, default latest
#   --repo <owner/repo>  GitHub repository, default __PROXYPANEL_RELEASE_REPO__
#   --uninstall          Uninstall the hub service (keep data)
#   --purge              With --uninstall, also remove data and configs
#   -h, --help           Show this help

set -euo pipefail

# ---------------------------------------------------------------------------
# Logging helpers
# ---------------------------------------------------------------------------
log() {
    echo "[proxy-panel] $*"
}

err() {
    echo "[proxy-panel] 错误: $*" >&2
}

# ---------------------------------------------------------------------------
# Help
# ---------------------------------------------------------------------------
show_help() {
    sed -n '2,16p' "$0"
}

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------
VERSION="latest"
REPO="__PROXYPANEL_RELEASE_REPO__"
UNINSTALL=false
PURGE=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --version)
            VERSION="$2"
            shift 2
            ;;
        --repo)
            REPO="$2"
            shift 2
            ;;
        --uninstall)
            UNINSTALL=true
            shift
            ;;
        --purge)
            PURGE=true
            shift
            ;;
        -h|--help)
            show_help
            exit 0
            ;;
        *)
            err "未知参数: $1"
            show_help
            exit 1
            ;;
    esac
done

# ---------------------------------------------------------------------------
# 1. Root check
# ---------------------------------------------------------------------------
if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
    err "请使用 root 权限运行此脚本（sudo）"
    exit 1
fi

# ---------------------------------------------------------------------------
# 2. Architecture detection
# ---------------------------------------------------------------------------
ARCH_RAW="$(uname -m)"
case "$ARCH_RAW" in
    x86_64)
        ARCH="x86_64"
        ;;
    aarch64|arm64)
        ARCH="aarch64"
        ;;
    *)
        err "不支持的架构: $ARCH_RAW（仅支持 x86_64 / aarch64）"
        exit 1
        ;;
esac
log "检测到架构: $ARCH"

# ---------------------------------------------------------------------------
# 3. Resolve repository
# ---------------------------------------------------------------------------
if [[ "$REPO" == *"__"* ]]; then
    REPO="ybakiame/multi-proxy-panel"
    log "使用默认仓库: $REPO"
fi

# ---------------------------------------------------------------------------
# 4. Compute download URLs
# ---------------------------------------------------------------------------
if [[ "$VERSION" == "latest" ]]; then
    DOWNLOAD_BASE="https://github.com/${REPO}/releases/latest/download"
else
    DOWNLOAD_BASE="https://github.com/${REPO}/releases/download/${VERSION}"
fi

CLI_TARBALL="proxy-panel-cli-linux-${ARCH}.tar.gz"
SHA256_FILE="SHA256SUMS"

# ---------------------------------------------------------------------------
# 5. Download and verify CLI tarball
# ---------------------------------------------------------------------------
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

log "下载 ${CLI_TARBALL}..."
if ! curl -fsSL -o "${TMP_DIR}/${CLI_TARBALL}" "${DOWNLOAD_BASE}/${CLI_TARBALL}"; then
    err "下载 ${CLI_TARBALL} 失败"
    exit 1
fi

log "下载 ${SHA256_FILE}..."
if ! curl -fsSL -o "${TMP_DIR}/${SHA256_FILE}" "${DOWNLOAD_BASE}/${SHA256_FILE}"; then
    err "下载 ${SHA256_FILE} 失败"
    exit 1
fi

log "校验 SHA256..."
if ! (cd "$TMP_DIR" && sha256sum -c "$SHA256_FILE" --ignore-missing); then
    err "SHA256 校验失败，文件可能被篡改"
    exit 1
fi
log "校验通过"

# ---------------------------------------------------------------------------
# 6. Extract and install CLI binary
# ---------------------------------------------------------------------------
log "解压并安装 proxy-panel CLI..."
tar -xzf "${TMP_DIR}/${CLI_TARBALL}" -C "$TMP_DIR"
install -m 755 "${TMP_DIR}/proxy-panel" /usr/local/bin/proxy-panel
log "已安装 /usr/local/bin/proxy-panel"

# ---------------------------------------------------------------------------
# 7. Delegate to proxy-panel CLI
# ---------------------------------------------------------------------------
if [[ "$UNINSTALL" == true ]]; then
    log "转发到 proxy-panel uninstall hub..."
    if [[ "$PURGE" == true ]]; then
        exec /usr/local/bin/proxy-panel uninstall hub --purge
    else
        exec /usr/local/bin/proxy-panel uninstall hub
    fi
else
    log "转发到 proxy-panel install hub..."
    PROXY_PANEL_ARGS=(
        install hub
    )
    if [[ -n "$VERSION" ]]; then
        PROXY_PANEL_ARGS+=(--version "$VERSION")
    fi
    if [[ -n "$REPO" ]]; then
        PROXY_PANEL_ARGS+=(--repo "$REPO")
    fi
    exec /usr/local/bin/proxy-panel "${PROXY_PANEL_ARGS[@]}"
fi
