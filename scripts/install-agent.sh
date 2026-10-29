#!/usr/bin/env bash
# ProxyPanel Agent bootstrap script
# Downloads the CLI binary then delegates to `proxy-panel install agent`.
#
# Usage:
#   curl -fsSL https://<hub>/install.sh | bash -s -- --hub-url https://<hub>:443 --token xxx [--agent-id <uuid>] [--name mynode] [--version latest] [--repo owner/repo]
#   bash install-agent.sh --hub-url https://<hub>:443 --token <token> --name mynode
#
# Options:
#   --hub-url <url>      Hub gRPC address (required unless --uninstall)
#   --token <token>      Node token (required unless --uninstall)
#   --agent-id <uuid>    Node UUID (optional)
#   --name <name>        Node display name (optional, defaults to hostname)
#   --version <ver>      Release version, default latest
#   --repo <owner/repo>  GitHub repository, default __PROXYPANEL_RELEASE_REPO__
#   --uninstall          Uninstall the agent service (keep data)
#   --purge              With --uninstall, also remove data and configs
#   -h, --help           Show this help

set -euo pipefail
umask 077

CURL_OPTIONS=(--proto '=https' --tlsv1.2 -fsSL --connect-timeout 10 --retry 3 --retry-delay 2)

download() {
    curl "${CURL_OPTIONS[@]}" -o "$1" "$2"
}

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
    sed -n '2,20p' "$0"
}

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------
HUB_URL=""
TOKEN=""
AGENT_ID=""
AGENT_NAME=""
VERSION="latest"
REPO="__PROXYPANEL_RELEASE_REPO__"
UNINSTALL=false
PURGE=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --hub-url)
            HUB_URL="$2"
            shift 2
            ;;
        --token)
            TOKEN="$2"
            shift 2
            ;;
        --agent-id)
            AGENT_ID="$2"
            shift 2
            ;;
        --name)
            AGENT_NAME="$2"
            shift 2
            ;;
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
# 1. Root check and input validation
# ---------------------------------------------------------------------------
if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
    err "请使用 root 权限运行此脚本（sudo）"
    exit 1
fi

if [[ "$UNINSTALL" != true ]]; then
    if [[ -z "$HUB_URL" || -z "$TOKEN" ]]; then
        err "安装 Agent 必须提供 --hub-url 和 --token"
        exit 1
    fi
    if [[ "$HUB_URL" == *$'\r'* || "$HUB_URL" == *$'\n'* ]]; then
        err "Hub URL 不能包含换行符"
        exit 1
    fi
    case "$HUB_URL" in
        https://*|http://127.0.0.1|http://127.0.0.1:*|http://localhost|http://localhost:*) ;;
        *)
            err "Hub URL 必须使用 HTTPS；仅本机开发可使用 http://127.0.0.1 或 http://localhost"
            exit 1
            ;;
    esac
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
if ! download "${TMP_DIR}/${CLI_TARBALL}" "${DOWNLOAD_BASE}/${CLI_TARBALL}"; then
    err "下载 ${CLI_TARBALL} 失败"
    exit 1
fi

log "下载 ${SHA256_FILE}..."
if ! download "${TMP_DIR}/${SHA256_FILE}" "${DOWNLOAD_BASE}/${SHA256_FILE}"; then
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
    log "转发到 proxy-panel uninstall agent..."
    if [[ "$PURGE" == true ]]; then
        exec /usr/local/bin/proxy-panel uninstall agent --purge
    else
        exec /usr/local/bin/proxy-panel uninstall agent
    fi
else
    log "转发到 proxy-panel install agent..."
    PROXY_PANEL_ARGS=(
        install agent
        --hub-url "$HUB_URL"
        --token "$TOKEN"
    )
    if [[ -n "$AGENT_ID" ]]; then
        PROXY_PANEL_ARGS+=(--agent-id "$AGENT_ID")
    fi
    if [[ -n "$AGENT_NAME" ]]; then
        PROXY_PANEL_ARGS+=(--name "$AGENT_NAME")
    fi
    if [[ -n "$VERSION" ]]; then
        PROXY_PANEL_ARGS+=(--version "$VERSION")
    fi
    if [[ -n "$REPO" ]]; then
        PROXY_PANEL_ARGS+=(--repo "$REPO")
    fi
    exec /usr/local/bin/proxy-panel "${PROXY_PANEL_ARGS[@]}"
fi
