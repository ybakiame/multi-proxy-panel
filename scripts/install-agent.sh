#!/usr/bin/env bash
# ProxyPanel Agent 一键安装脚本
# Usage:
#   curl -fsSL https://<hub>/install.sh | bash -s -- --hub-url http://hub:50052 --token xxx [--agent-id <uuid>] [--name mynode] [--version v0.3.3]
#   bash install-agent.sh --hub-url http://1.2.3.4:50052 --token <token> --name mynode
#
# Options:
#   --hub-url <url>      Hub gRPC 地址（必填，--uninstall 时除外）
#   --token <token>      节点 token（必填）
#   --agent-id <uuid>    节点 UUID（可选）
#   --name <name>        节点显示名（可选，默认 hostname）
#   --version <ver>      版本，默认 latest
#   --repo <owner/repo>  GitHub 仓库，默认 __PROXYPANEL_RELEASE_REPO__
#   --uninstall          卸载服务（保留数据）
#   --purge              配合 --uninstall 删除数据与配置
#   -h, --help           显示帮助
#
# Example:
#   bash install-agent.sh --hub-url http://192.168.1.100:50052 --token abc123 --name node-01 --version v0.3.3

set -euo pipefail

# -----------------------------------------------------------------------------
# 日志输出
# -----------------------------------------------------------------------------
log() {
    echo "[proxy-panel] $*"
}

err() {
    echo "[proxy-panel] 错误: $*" >&2
}

# -----------------------------------------------------------------------------
# 帮助信息
# -----------------------------------------------------------------------------
show_help() {
    sed -n '2,20p' "$0"
}

# -----------------------------------------------------------------------------
# 参数解析
# -----------------------------------------------------------------------------
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

# -----------------------------------------------------------------------------
# 1. root 权限检查
# -----------------------------------------------------------------------------
if [[ "${EUID:-$(id -u)}" -ne 0 ]]; then
    err "请使用 root 权限运行此脚本（sudo）"
    exit 1
fi

# -----------------------------------------------------------------------------
# 卸载模式
# -----------------------------------------------------------------------------
if [[ "$UNINSTALL" == true ]]; then
    log "开始卸载 ProxyPanel Agent..."

    if systemctl list-unit-files proxy-panel-agent.service &>/dev/null; then
        log "停止并禁用 proxy-panel-agent 服务..."
        systemctl stop proxy-panel-agent || true
        systemctl disable proxy-panel-agent || true
    fi

    if [[ -f /etc/systemd/system/proxy-panel-agent.service ]]; then
        rm -f /etc/systemd/system/proxy-panel-agent.service
        systemctl daemon-reload || true
        log "已删除 systemd unit 文件"
    fi

    if [[ -f /usr/local/bin/proxy-panel-agent ]]; then
        rm -f /usr/local/bin/proxy-panel-agent
        log "已删除二进制文件"
    fi

    if [[ "$PURGE" == true ]]; then
        log "清理数据与配置目录..."
        rm -rf /var/lib/proxy-panel
        rm -rf /opt/proxy-panel
        rm -rf /etc/proxy-panel
        log "已删除 /var/lib/proxy-panel、/opt/proxy-panel、/etc/proxy-panel"
    else
        log "保留数据目录 /var/lib/proxy-panel（如需删除请加上 --purge）"
    fi

    log "卸载完成"
    exit 0
fi

# -----------------------------------------------------------------------------
# 2. 必填参数校验
# -----------------------------------------------------------------------------
if [[ -z "$HUB_URL" ]]; then
    err "缺少必填参数 --hub-url"
    show_help
    exit 1
fi

if [[ -z "$TOKEN" ]]; then
    err "缺少必填参数 --token"
    show_help
    exit 1
fi

# 默认节点名
if [[ -z "$AGENT_NAME" ]]; then
    AGENT_NAME="$(hostname)"
fi

# 回退 repo 占位符
if [[ "$REPO" == *"__"* ]]; then
    REPO="ybakiame/multi-proxy-panel"
    log "使用默认仓库: $REPO"
fi

log "开始安装 ProxyPanel Agent..."
log "Hub 地址: $HUB_URL"
log "节点名称: $AGENT_NAME"
log "版本: $VERSION"

# -----------------------------------------------------------------------------
# 3. 架构检测
# -----------------------------------------------------------------------------
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

# -----------------------------------------------------------------------------
# 4. glibc 版本检查
# -----------------------------------------------------------------------------
if command -v ldd &>/dev/null; then
    GLIBC_VER="$(ldd --version 2>/dev/null | head -n1 | grep -oP '[0-9]+\.[0-9]+' || true)"
    if [[ -n "$GLIBC_VER" ]]; then
        GLIBC_MAJOR="${GLIBC_VER%%.*}"
        GLIBC_MINOR="${GLIBC_VER#*.}"
        if [[ "$GLIBC_MAJOR" -lt 2 ]] || { [[ "$GLIBC_MAJOR" -eq 2 ]] && [[ "$GLIBC_MINOR" -lt 35 ]]; }; then
            log "警告: 当前 glibc 版本为 $GLIBC_VER，构建产物基于 ubuntu-22.04 (glibc 2.35)，可能无法运行"
        else
            log "glibc 版本: $GLIBC_VER"
        fi
    fi
fi

# -----------------------------------------------------------------------------
# 5. 下载与校验
# -----------------------------------------------------------------------------
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

if [[ "$VERSION" == "latest" ]]; then
    DOWNLOAD_BASE="https://github.com/${REPO}/releases/latest/download"
else
    DOWNLOAD_BASE="https://github.com/${REPO}/releases/download/${VERSION}"
fi

TARBALL="proxy-panel-agent-linux-${ARCH}.tar.gz"
SHA256_FILE="SHA256SUMS"

log "下载 ${TARBALL}..."
if ! curl -fsSL -o "${TMP_DIR}/${TARBALL}" "${DOWNLOAD_BASE}/${TARBALL}"; then
    err "下载 ${TARBALL} 失败"
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

# -----------------------------------------------------------------------------
# 6. 安装二进制
# -----------------------------------------------------------------------------
log "解压并安装二进制..."
tar -xzf "${TMP_DIR}/${TARBALL}" -C "$TMP_DIR"

# 若服务在运行则先停止
if systemctl is-active --quiet proxy-panel-agent 2>/dev/null; then
    log "检测到 proxy-panel-agent 正在运行，先停止服务..."
    systemctl stop proxy-panel-agent || true
fi

install -m 755 "${TMP_DIR}/proxy-panel-agent" /usr/local/bin/proxy-panel-agent
log "已安装 /usr/local/bin/proxy-panel-agent"

# -----------------------------------------------------------------------------
# 7. 创建用户与目录
# -----------------------------------------------------------------------------
if ! id -u proxypanel &>/dev/null; then
    log "创建系统用户 proxypanel..."
    useradd -r -s /sbin/nologin proxypanel
else
    log "系统用户 proxypanel 已存在，跳过创建"
fi

mkdir -p /var/lib/proxy-panel/agent
mkdir -p /opt/proxy-panel/bin
mkdir -p /etc/proxy-panel

chown -R proxypanel:proxypanel /var/lib/proxy-panel
chown -R proxypanel:proxypanel /opt/proxy-panel
chown proxypanel:proxypanel /etc/proxy-panel

# -----------------------------------------------------------------------------
# 8. 写入环境配置文件
# -----------------------------------------------------------------------------
log "写入 /etc/proxy-panel/agent.env..."
{
    echo "PROXYPANEL_HUB_URL=${HUB_URL}"
    echo "PROXYPANEL_AGENT_TOKEN=${TOKEN}"
    if [[ -n "$AGENT_ID" ]]; then
        echo "PROXYPANEL_AGENT_ID=${AGENT_ID}"
    fi
    echo "PROXYPANEL_AGENT_NAME=${AGENT_NAME}"
    echo "RUST_LOG=proxy_panel_agent=info"
} > /etc/proxy-panel/agent.env

chmod 600 /etc/proxy-panel/agent.env
chown proxypanel:proxypanel /etc/proxy-panel/agent.env
log "环境配置已写入"

# -----------------------------------------------------------------------------
# 9. 写入 systemd unit
# -----------------------------------------------------------------------------
log "写入 systemd unit..."
cat > /etc/systemd/system/proxy-panel-agent.service << 'EOF'
[Unit]
Description=ProxyPanel Agent
After=network.target
Wants=network.target

[Service]
Type=simple
User=proxypanel
Group=proxypanel
WorkingDirectory=/opt/proxy-panel

Environment=RUST_LOG=proxy_panel_agent=info
EnvironmentFile=-/etc/proxy-panel/agent.env

ExecStart=/usr/local/bin/proxy-panel-agent \
    --hub-url ${PROXYPANEL_HUB_URL} \
    --token ${PROXYPANEL_AGENT_TOKEN} \
    --data-dir /var/lib/proxy-panel/agent \
    --bin-dir /opt/proxy-panel/bin

Restart=on-failure
RestartSec=5

# Security hardening
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
ReadWritePaths=/var/lib/proxy-panel /opt/proxy-panel/bin

# Allow sing-box child process to bind ACME challenge ports (80/443).
AmbientCapabilities=CAP_NET_BIND_SERVICE

[Install]
WantedBy=multi-user.target
EOF
log "systemd unit 已写入"

# -----------------------------------------------------------------------------
# 10. 启动服务
# -----------------------------------------------------------------------------
log "重载 systemd 并启动服务..."
systemctl daemon-reload
systemctl enable --now proxy-panel-agent

log "等待服务启动..."
sleep 2

if systemctl is-active --quiet proxy-panel-agent; then
    log "ProxyPanel Agent 安装成功，服务运行中"
else
    err "服务启动失败，最近 20 行日志如下："
    journalctl -u proxy-panel-agent --no-pager -n 20 || true
    exit 1
fi
