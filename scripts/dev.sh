#!/usr/bin/env bash
set -euo pipefail

# ProxyPanel 开发环境一键管理脚本
# 用法: ./scripts/dev.sh [start|stop|restart|status]
#
# 使用 nohup 启动所有服务，避免后台任务超时问题。

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
PID_FILE="$PROJECT_ROOT/scripts/.dev-pids"
LOG_DIR="$PROJECT_ROOT/scripts/.dev-logs"

# 配置
HUB_HTTP_PORT=8081
HUB_GRPC_PORT=50052
WEB_PORT=8085
DB_URL="postgres://proxypanel:proxypanel@localhost/proxypanel"
HUB_LOG="$LOG_DIR/hub.log"
AGENT_LOG="$LOG_DIR/agent.log"
WEB_LOG="$LOG_DIR/web.log"

# 开发环境默认配置路径
HUB_CONFIG="${HUB_CONFIG:-$PROJECT_ROOT/config/hub.toml}"
AGENT_DATA_DIR="${AGENT_DATA_DIR:-/tmp/proxypanel-agent}"

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

log_info()  { echo -e "${GREEN}[INFO]${NC}  $*"; }
log_warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
log_error() { echo -e "${RED}[ERROR]${NC} $*"; }
log_step()  { echo -e "${BLUE}[STEP]${NC}  $*"; }

ensure_dirs() {
    mkdir -p "$LOG_DIR"
}

check_protoc() {
    if ! command -v protoc &> /dev/null; then
        log_error "未找到 protoc (Protocol Buffers 编译器)"
        echo ""
        log_info "Hub 编译依赖 protoc，请根据系统选择以下方式安装:"
        echo ""
        echo "  Debian/Ubuntu:  sudo apt-get install protobuf-compiler"
        echo "  Arch Linux:     sudo pacman -S protobuf"
        echo "  macOS:          brew install protobuf"
        echo "  其他系统:       https://github.com/protocolbuffers/protobuf/releases"
        echo ""
        log_info "安装完成后重新运行此脚本"
        exit 1
    fi
}

# ========== 启动 ==========

cmd_start() {
    if [ -f "$PID_FILE" ]; then
        log_warn "PID 文件已存在，可能已有服务在运行"
        cmd_status
        echo ""
        read -rp "是否先停止现有服务再继续? [y/N] " confirm
        if [[ "$confirm" =~ ^[Yy]$ ]]; then
            cmd_stop
        else
            log_info "取消启动"
            exit 0
        fi
    fi

    ensure_dirs
    check_protoc
    log_step "启动 ProxyPanel 开发环境..."
    echo ""

    # 1. 启动 PostgreSQL
    log_step "[1/5] 启动 PostgreSQL..."
    cd "$PROJECT_ROOT"
    # docker compose 会验证整个 compose 文件的变量，即使只启动 postgres。
    # 开发环境为 hub/agent 的必需变量提供占位值，避免启动失败。
    export PROXYPANEL_BOOTSTRAP_API_KEY="${PROXYPANEL_BOOTSTRAP_API_KEY:-dev-bootstrap-key-change-me}"
    export PROXYPANEL_AGENT_TOKEN="${PROXYPANEL_AGENT_TOKEN:-dev-agent-token-change-me}"

    if docker compose ps postgres 2>/dev/null | grep -q "running\|Up"; then
        log_info "PostgreSQL 已在运行"
    else
        if ! docker compose up -d postgres >/dev/null 2>&1; then
            log_error "PostgreSQL 启动失败，请检查 docker compose 输出"
            exit 1
        fi
        # 等待 PostgreSQL 就绪
        for i in $(seq 1 30); do
            if docker compose exec -T postgres pg_isready -U proxypanel >/dev/null 2>&1; then
                break
            fi
            sleep 1
        done
        log_info "PostgreSQL 启动完成"
    fi

    # 2. 初始化数据库（如果表不存在）
    log_step "[2/5] 检查数据库..."
    cd "$PROJECT_ROOT"
    if ! cargo run --quiet --bin proxy-panel -- diagnose --database-url "$DB_URL" >/dev/null 2>&1; then
        log_info "初始化数据库..."
        cargo run --quiet --bin proxy-panel -- init-db --database-url "$DB_URL" >/dev/null 2>&1
        log_info "数据库初始化完成"
    else
        log_info "数据库已就绪"
    fi

    # 3. 启动 Hub
    log_step "[3/5] 启动 Hub (HTTP:$HUB_HTTP_PORT, gRPC:$HUB_GRPC_PORT)..."
    cd "$PROJECT_ROOT"
    # 确保开发配置文件存在
    if [ ! -f "$HUB_CONFIG" ]; then
        mkdir -p "$(dirname "$HUB_CONFIG")"
        cat > "$HUB_CONFIG" <<EOF
listen = "0.0.0.0:$HUB_HTTP_PORT"
grpc_listen = "0.0.0.0:$HUB_GRPC_PORT"
database_url = "$DB_URL"
static_dir = "apps/panel/dist"
cors_origins = "http://localhost:$WEB_PORT,http://127.0.0.1:$WEB_PORT"
auto_register_agents = true
EOF
        log_info "已生成开发配置文件: $HUB_CONFIG"
    fi
    nohup bash -c "
        RUST_LOG=proxy_panel_hub=info,tower_http=debug \
        cargo run --bin proxy-panel-hub -- \
            --config '$HUB_CONFIG' \
            --static-dir apps/panel/dist \
            --listen 0.0.0.0:$HUB_HTTP_PORT \
            --grpc-listen 0.0.0.0:$HUB_GRPC_PORT
    " > "$HUB_LOG" 2>&1 &
    HUB_PID=$!
    echo "hub:$HUB_PID" >> "$PID_FILE"

    # 等待 Hub 就绪
    for i in $(seq 1 30); do
        if curl -s "http://localhost:$HUB_HTTP_PORT/health" >/dev/null 2>&1; then
            break
        fi
        sleep 1
    done
    log_info "Hub 启动完成 (PID: $HUB_PID)"

    # 4. 启动 Agent
    log_step "[4/5] 启动 Agent..."
    cd "$PROJECT_ROOT"
    BIN_DIR="$PROJECT_ROOT/scripts/.dev-bin"
    mkdir -p "$BIN_DIR"
    nohup bash -c "
        RUST_LOG=proxy_panel_agent=info \
        cargo run --bin proxy-panel-agent -- \
            --hub-url \"http://localhost:$HUB_GRPC_PORT\" \
            --name \"dev-agent-node\" \
            --data-dir \"$AGENT_DATA_DIR\" \
            --bin-dir \"$BIN_DIR\"
    " > "$AGENT_LOG" 2>&1 &
    AGENT_PID=$!
    echo "agent:$AGENT_PID" >> "$PID_FILE"
    log_info "Agent 启动完成 (PID: $AGENT_PID)"

    if [ ! -x "$BIN_DIR/sing-box" ] && [ ! -x "$BIN_DIR/xray" ]; then
        echo ""
        log_warn "未在 $BIN_DIR 找到 sing-box 或 xray 可执行文件"
        log_warn "推送配置功能需要这些二进制文件。请下载后放到该目录："
        log_info "  sing-box: https://github.com/SagerNet/sing-box/releases"
        log_info "  xray:     https://github.com/XTLS/Xray-core/releases"
        echo ""
    fi

    # 5. 启动 Web 前端
    log_step "[5/5] 启动 Web 前端 (http://localhost:$WEB_PORT)..."
    cd "$PROJECT_ROOT/apps/panel"
    if [ ! -d "node_modules" ]; then
        log_info "安装前端依赖..."
        bun install >/dev/null 2>npm install >/dev/null 2>&11
    fi
    # 前端开发服务器需要知道 Hub API 地址，否则默认使用自身 origin 导致 404。
    nohup bash -c "PROXYPANEL_API_URL=http://127.0.0.1:$HUB_HTTP_PORT bun run dev -- --host 127.0.0.1 --port $WEB_PORT" > "$WEB_LOG" 2>&1 &
    WEB_PID=$!
    echo "web:$WEB_PID" >> "$PID_FILE"
    log_info "Web 前端启动完成 (PID: $WEB_PID)"

    echo ""
    log_info "========================================"
    log_info "ProxyPanel 开发环境已全部启动!"
    log_info "========================================"
    echo ""
    log_info "管理端面板: http://localhost:$WEB_PORT"
    log_info "Hub API:    http://localhost:$HUB_HTTP_PORT"
    log_info "Hub gRPC:   http://localhost:$HUB_GRPC_PORT"
    echo ""
    # 提取并展示 bootstrap API key
    sleep 1
    # Bootstrap key 通过 stderr 输出且不会被记录到日志文件。
    # 这里尝试从日志文件中获取；如果未捕获到，提示用户查看终端输出。
    BOOTSTRAP_KEY=$(grep -oP 'BOOTSTRAP API KEY created (one-time): \K[A-Za-z0-9+/=]+' "$HUB_LOG" | tail -1 || true)
    if [ -n "$BOOTSTRAP_KEY" ]; then
        log_warn "首次登录请使用下方 Bootstrap API Key："
        log_warn "  $BOOTSTRAP_KEY"
        log_warn "登录后可在 系统管理 -> API 密钥 中轮换。"
    else
        log_info "未从日志中捕获到 Bootstrap API Key。"
        log_info "如果这是首次启动，key 已输出到启动 Hub 的终端/日志；请查看 $HUB_LOG"
    fi
    echo ""
    log_info "日志文件:"
    log_info "  Hub:   $HUB_LOG"
    log_info "  Agent: $AGENT_LOG"
    log_info "  Web:   $WEB_LOG"
    echo ""
    log_info "查看实时日志: tail -f $LOG_DIR/*.log"
    log_info "停止所有服务: ./scripts/dev.sh stop"
    echo ""
}

# ========== 停止 ==========

cmd_stop() {
    if [ ! -f "$PID_FILE" ]; then
        log_warn "PID 文件不存在，尝试查找残留进程..."
        kill_residual
        return
    fi

    log_step "停止 ProxyPanel 开发环境..."
    echo ""

    # 按相反顺序停止：web -> agent -> hub
    while IFS=: read -r name pid; do
        if kill -0 "$pid" 2>/dev/null; then
            log_step "停止 $name (PID: $pid)..."
            kill "$pid" 2>/dev/null || true
            # 等待进程退出
            for i in $(seq 1 10); do
                if ! kill -0 "$pid" 2>/dev/null; then
                    break
                fi
                sleep 0.5
            done
            # 强制终止
            if kill -0 "$pid" 2>/dev/null; then
                log_warn "$name 未响应，强制终止..."
                kill -9 "$pid" 2>/dev/null || true
            fi
            log_info "$name 已停止"
        else
            log_warn "$name (PID: $pid) 已不在运行"
        fi
    done < <(tac "$PID_FILE")

    rm -f "$PID_FILE"

    # 停止 PostgreSQL
    log_step "停止 PostgreSQL..."
    cd "$PROJECT_ROOT"
    # 同样需要占位值才能执行 docker compose down
    export PROXYPANEL_BOOTSTRAP_API_KEY="${PROXYPANEL_BOOTSTRAP_API_KEY:-dev-bootstrap-key-change-me}"
    export PROXYPANEL_AGENT_TOKEN="${PROXYPANEL_AGENT_TOKEN:-dev-agent-token-change-me}"
    docker compose down >/dev/null 2>&1 || true
    log_info "PostgreSQL 已停止"

    # 保留开发配置文件，不删除
    echo ""
    log_info "所有服务已停止"
}

kill_residual() {
    log_step "清理残留进程..."
    # 查找并停止所有 proxypanel 相关进程
    local pids
    pids=$(ps aux | grep -E 'proxy-panel-hub|proxy-panel-agent|npm run dev' | grep -v grep | awk '{print $2}' || true)
    if [ -n "$pids" ]; then
        echo "$pids" | xargs kill -9 2>/dev/null || true
        log_info "已清理残留进程"
    else
        log_info "无残留进程"
    fi
    rm -f "$PID_FILE"
}

# ========== 状态 ==========

cmd_status() {
    echo ""
    log_info "========================================"
    log_info "ProxyPanel 开发环境状态"
    log_info "========================================"
    echo ""

    local all_ok=true

    # PostgreSQL
    cd "$PROJECT_ROOT"
    export PROXYPANEL_BOOTSTRAP_API_KEY="${PROXYPANEL_BOOTSTRAP_API_KEY:-dev-bootstrap-key-change-me}"
    export PROXYPANEL_AGENT_TOKEN="${PROXYPANEL_AGENT_TOKEN:-dev-agent-token-change-me}"
    if docker compose ps postgres 2>/dev/null | grep -q "running\|Up"; then
        log_info "✅ PostgreSQL   running (docker)"
    else
        log_error "❌ PostgreSQL   stopped"
        all_ok=false
    fi

    # Hub
    if ss -tlnp 2>/dev/null | grep -q ":$HUB_HTTP_PORT"; then
        local hub_pid=""
        hub_pid=$(ss -tlnp 2>/dev/null | grep ":$HUB_HTTP_PORT" | grep -oP 'pid=\K[0-9]+' | head -1)
        log_info "✅ Hub          running (PID: ${hub_pid:-?}, HTTP:$HUB_HTTP_PORT, gRPC:$HUB_GRPC_PORT)"
    else
        log_error "❌ Hub          stopped"
        all_ok=false
    fi

    # Agent
    local agent_pid=""
    agent_pid=$(pgrep -f 'proxy-panel-agent' || true)
    if [ -n "$agent_pid" ]; then
        log_info "✅ Agent        running (PID: $agent_pid)"
    else
        log_error "❌ Agent        stopped"
        all_ok=false
    fi

    # Web
    if ss -tlnp 2>/dev/null | grep -q ":$WEB_PORT"; then
        local web_pid=""
        web_pid=$(ss -tlnp 2>/dev/null | grep ":$WEB_PORT" | grep -oP 'pid=\K[0-9]+' | head -1)
        log_info "✅ Web Frontend running (PID: ${web_pid:-?}, http://localhost:$WEB_PORT)"
    else
        log_error "❌ Web Frontend stopped"
        all_ok=false
    fi

    echo ""
    if [ "$all_ok" = true ]; then
        log_info "管理端面板: http://localhost:$WEB_PORT"
        log_info "Hub API:    http://localhost:$HUB_HTTP_PORT"
        log_info "Hub gRPC:   http://localhost:$HUB_GRPC_PORT"
    fi
}

# ========== 主入口 ==========

main() {
    case "${1:-status}" in
        start)
            cmd_start
            ;;
        stop)
            cmd_stop
            ;;
        restart)
            cmd_stop
            sleep 1
            cmd_start
            ;;
        status)
            cmd_status
            ;;
        *)
            echo "用法: $0 [start|stop|restart|status]"
            echo ""
            echo "  start    启动所有开发服务 (PostgreSQL + Hub + Agent + Web)"
            echo "  stop     停止所有开发服务"
            echo "  restart  重启所有开发服务"
            echo "  status   查看服务状态"
            echo ""
            echo "环境变量:"
            echo "  HUB_CONFIG      Hub 配置文件路径 (默认: $HUB_CONFIG)"
            echo "  AGENT_DATA_DIR  Agent 数据目录 (默认: $AGENT_DATA_DIR)"
            exit 1
            ;;
    esac
}

main "$@"
