# ProxyPanel 部署指南

本文档描述如何在生产环境中部署 ProxyPanel 的各个组件。

---

## 目录

1. [部署要求](#部署要求)
2. [Docker Compose 部署（推荐）](#docker-compose-部署推荐)
3. [节点一键安装（推荐）](#节点一键安装推荐)
4. [手动部署](#手动部署)
5. [高可用部署](#高可用部署)
6. [安全配置](#安全配置)
7. [监控与日志](#监控与日志)
8. [备份与恢复](#备份与恢复)
9. [升级指南](#升级指南)
10. [故障排查](#故障排查)
11. [CI/CD 产物说明](#cicd-产物说明)

---

## 部署要求

### Hub 服务器

| 资源 | 最低配置 | 推荐配置 |
|------|----------|----------|
| CPU | 2 核 | 4 核 |
| 内存 | 2 GB | 4 GB |
| 磁盘 | 20 GB SSD | 50 GB SSD |
| 网络 | 100 Mbps | 1 Gbps |
| 操作系统 | Ubuntu 22.04 LTS | Ubuntu 22.04/24.04 LTS |

### Agent 服务器（节点）

| 资源 | 最低配置 | 推荐配置 |
|------|----------|----------|
| CPU | 1 核 | 2 核 |
| 内存 | 512 MB | 1 GB |
| 磁盘 | 10 GB | 20 GB |
| 网络 | 100 Mbps | 500 Mbps |

### 依赖服务

| 服务 | 版本 | 说明 |
|------|------|------|
| PostgreSQL | 15+ | 数据持久化 |
| Redis | 7+ | 可选，用于缓存和会话 |
| Nginx / Caddy | 最新 | 反向代理和 TLS |

---

## Docker Compose 部署（推荐）

### 1. 准备环境

```bash
# 创建部署目录
mkdir -p /opt/proxypanel
cd /opt/proxypanel

# 下载 compose 文件
curl -O https://raw.githubusercontent.com/ybakiame/multi-proxy-panel/main/docker-compose.yml
curl -O https://raw.githubusercontent.com/ybakiame/multi-proxy-panel/main/Dockerfile.hub
curl -O https://raw.githubusercontent.com/ybakiame/multi-proxy-panel/main/Dockerfile.agent
```

### 2. 配置环境变量

创建 `.env` 文件：

```bash
cat > .env << 'EOF'
# Database
POSTGRES_USER=proxypanel
POSTGRES_PASSWORD=CHANGE_THIS_TO_STRONG_PASSWORD
POSTGRES_DB=proxypanel

# Hub
PROXYPANEL_HUB_LISTEN=0.0.0.0:8080
PROXYPANEL_GRPC_LISTEN=0.0.0.0:50052
PROXYPANEL_DATABASE_URL=postgres://proxypanel:CHANGE_THIS_TO_STRONG_PASSWORD@postgres/proxypanel
RUST_LOG=proxy_panel_hub=info,tower_http=info

# Agent (if running agent on same host)
PROXYPANEL_PUBLIC_HTTP_URL=https://panel.example.com
PROXYPANEL_PUBLIC_GRPC_URL=https://grpc.example.com:50052
PROXYPANEL_RELEASE_REPO=ybakiame/multi-proxy-panel
EOF
```

> `public_http_url` 与 `public_grpc_url` 用于生成正确的安装脚本地址和 Agent 连接地址。若 Hub 位于反向代理后，必须显式配置这两项，否则 Hub 会尝试从请求头推断，可能产生错误的内网地址。

### 3. 启动服务

```bash
# 启动 Hub + PostgreSQL
docker compose up -d

# 查看日志
docker compose logs -f hub

# 初始化数据库（首次部署）
docker compose exec hub proxy-panel init-db \
  --database-url "$PROXYPANEL_DATABASE_URL"
```

### 4. 配置反向代理

#### Nginx 配置示例

```nginx
# /etc/nginx/sites-available/proxypanel
server {
    listen 443 ssl http2;
    server_name panel.example.com;

    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

    # Web 前端和 API
    location / {
        proxy_pass http://localhost:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}

# gRPC 代理（需要 http2）
server {
    listen 443 ssl http2;
    server_name grpc.example.com;

    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

    location / {
        grpc_pass grpc://localhost:50052;
        grpc_set_header Host $host;
    }
}
```

#### Caddy 配置示例

```caddyfile
# Caddyfile
panel.example.com {
    reverse_proxy localhost:8080
}

grpc.example.com {
    reverse_proxy h2c://localhost:50052
}
```

---

## 节点一键安装（推荐）

Hub 提供公开端点 `/install.sh` 伺服安装脚本，并在管理 API 中提供「生成安装指令」功能。这是最推荐的 Agent 部署方式。

### 前置配置

在 Hub 配置中设置以下新配置项（环境变量或配置文件）：

| 配置项 | 环境变量 | 说明 |
|--------|----------|------|
| `public_http_url` | `PROXYPANEL_PUBLIC_HTTP_URL` | Hub 对外 HTTP 地址，用于生成安装脚本 URL |
| `public_grpc_url` | `PROXYPANEL_PUBLIC_GRPC_URL` | Hub 对外 gRPC 地址，用于 Agent 连接 |
| `release_repo` | `PROXYPANEL_RELEASE_REPO` | GitHub 仓库地址，默认 `ybakiame/multi-proxy-panel` |

### 面板生成安装指令

1. 在 Web 前端 **节点管理** 页面，点击目标节点的「安装指令」按钮
2. 系统调用 `POST /api/v1/nodes/{id}/install-command`，自动：
   - 轮换该节点的 token（旧 token 立即失效）
   - 生成包含 `--hub-url`、`--token`、`--agent-id`、`--name`、`--version` 的 `curl \| bash` 命令
3. 复制弹窗中的命令，在节点服务器上以 root 执行即可

### 手动执行一键安装

若已知 token，也可直接在节点服务器上执行：

```bash
curl -fsSL https://panel.example.com/install.sh | bash -s -- \
  --hub-url "https://grpc.example.com:50052" \
  --token "your-agent-token" \
  --name "tokyo-01" \
  --version "v0.3.3"
```

### install-agent.sh 参数表

| 参数 | 必需 | 说明 |
|------|------|------|
| `--hub-url <url>` | ✅ | Hub gRPC 地址（`--uninstall` 时除外） |
| `--token <token>` | ✅ | 节点 token |
| `--agent-id <uuid>` | ❌ | 节点 UUID（面板生成时会自动填入） |
| `--name <name>` | ❌ | 节点显示名，默认取 `hostname` |
| `--version <ver>` | ❌ | 版本，默认 `latest` |
| `--repo <owner/repo>` | ❌ | GitHub 仓库，默认 `ybakiame/multi-proxy-panel` |
| `--uninstall` | ❌ | 卸载服务（保留数据） |
| `--purge` | ❌ | 配合 `--uninstall` 删除数据与配置 |
| `-h, --help` | ❌ | 显示帮助 |

### systemd 管理命令

安装完成后，Agent 以 systemd 服务运行：

```bash
# 查看状态
sudo systemctl status proxy-panel-agent

# 查看实时日志
sudo journalctl -u proxy-panel-agent -f

# 重启服务
sudo systemctl restart proxy-panel-agent

# 停止服务
sudo systemctl stop proxy-panel-agent
```

### 升级 Agent

重复执行安装命令即可升级：脚本会下载新版本、停止旧服务、替换二进制、重新启动。数据目录 `/var/lib/proxy-panel` 与配置 `/etc/proxy-panel/agent.env` 会被保留。

```bash
curl -fsSL https://panel.example.com/install.sh | bash -s -- \
  --hub-url "https://grpc.example.com:50052" \
  --token "your-new-token"
```

### 卸载 Agent

```bash
# 仅卸载服务，保留数据
curl -fsSL https://panel.example.com/install.sh | bash -s -- --uninstall

# 彻底卸载并删除所有数据
curl -fsSL https://panel.example.com/install.sh | bash -s -- --uninstall --purge
```

---

## 手动部署

### 1. 编译二进制文件

```bash
# 在开发机器上编译
git clone https://github.com/ybakiame/multi-proxy-panel.git
cd proxy-panel

# 编译 Release 版本
cargo build --release --workspace

# 产物路径:
# target/release/proxy-panel-hub
# target/release/proxy-panel-agent
# target/release/proxy-panel
```

### 2. 部署 Hub

```bash
# 在 Hub 服务器上
sudo mkdir -p /opt/proxypanel/{bin,config,web}
sudo cp target/release/proxy-panel-hub /opt/proxypanel/bin/
sudo cp target/release/proxy-panel /opt/proxypanel/bin/

# 创建配置文件
sudo tee /opt/proxypanel/config/hub.toml << 'EOF'
listen = "127.0.0.1:8081"
grpc_listen = "127.0.0.1:50052"
database_url = "postgres://proxypanel:PASSWORD@localhost/proxypanel"
static_dir = "/opt/proxypanel/web/dist"
EOF

# 复制前端文件
sudo cp -r crates/pp-web/dist/* /opt/proxypanel/web/dist/
```

### 3. 创建 systemd 服务

#### Hub 服务

```bash
sudo tee /etc/systemd/system/proxypanel-hub.service << 'EOF'
[Unit]
Description=ProxyPanel Hub
After=network.target postgresql.service

[Service]
Type=simple
User=proxypanel
Group=proxypanel
WorkingDirectory=/opt/proxypanel
ExecStart=/opt/proxypanel/bin/proxy-panel-hub --config /opt/proxypanel/config/hub.toml
Restart=always
RestartSec=5
Environment="RUST_LOG=proxy_panel_hub=info"

[Install]
WantedBy=multi-user.target
EOF
```

#### Agent 服务

```bash
sudo tee /etc/systemd/system/proxypanel-agent.service << 'EOF'
[Unit]
Description=ProxyPanel Agent
After=network.target

[Service]
Type=simple
User=root
Group=root
WorkingDirectory=/opt/proxypanel
ExecStart=/opt/proxypanel/bin/proxy-panel-agent \
  --hub-url "https://grpc.example.com" \
  --token "YOUR_AGENT_TOKEN" \
  --data-dir /var/lib/proxypanel \
  --bin-dir /usr/local/bin
Restart=always
RestartSec=10
Environment="RUST_LOG=proxy_panel_agent=info"

[Install]
WantedBy=multi-user.target
EOF
```

### 4. 启动服务

```bash
# 创建用户
sudo useradd -r -s /bin/false proxypanel
sudo chown -R proxypanel:proxypanel /opt/proxypanel

# 创建 Agent 数据目录
sudo mkdir -p /var/lib/proxypanel
sudo chmod 700 /var/lib/proxypanel

# 重载 systemd
sudo systemctl daemon-reload

# 启动 Hub
sudo systemctl enable --now proxypanel-hub

# 启动 Agent（在节点服务器上）
sudo systemctl enable --now proxypanel-agent

# 查看状态
sudo systemctl status proxypanel-hub
sudo journalctl -u proxypanel-hub -f
```

---

## 高可用部署

### 架构图

```
                    ┌─────────────┐
                    │   Caddy /   │
                    │   Nginx     │
                    │  (Load Bal) │
                    └──────┬──────┘
                           │
           ┌───────────────┼───────────────┐
           ▼               ▼               ▼
    ┌────────────┐  ┌────────────┐  ┌────────────┐
    │ Hub Instance│  │ Hub Instance│  │ Hub Instance│
    │     #1     │  │     #2     │  │     #3     │
    └──────┬─────┘  └──────┬─────┘  └──────┬─────┘
           │               │               │
           └───────────────┼───────────────┘
                           ▼
                    ┌─────────────┐
                    │ PostgreSQL  │
                    │  (Primary)  │
                    └─────────────┘
                           │
                    ┌──────┴──────┐
                    ▼             ▼
            ┌─────────────┐ ┌─────────────┐
            │  PostgreSQL │ │  PostgreSQL │
            │  (Replica)  │ │  (Replica)  │
            └─────────────┘ └─────────────┘
```

### 注意事项

1. **gRPC 连接**: Agent 与 Hub 建立长连接，负载均衡器需支持 TCP/HTTP2 会话保持
2. **状态同步**: Hub 实例间的 Agent 连接表不共享，推送配置时仅连接到特定 Hub 的 Agent 可接收
3. **数据库**: 使用 PostgreSQL 主从 + 连接池（PgBouncer）
4. **建议**: 当前版本建议单 Hub 实例部署，多实例架构将在后续版本完善

---

## 安全配置

### 1. 数据库安全

```sql
-- 创建专用数据库用户
CREATE USER proxypanel_app WITH PASSWORD 'strong_password';
GRANT CONNECT ON DATABASE proxypanel TO proxypanel_app;
GRANT USAGE ON SCHEMA public TO proxypanel_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO proxypanel_app;
GRANT USAGE ON ALL SEQUENCES IN SCHEMA public TO proxypanel_app;
```

### 2. 防火墙规则

```bash
# Hub 服务器
sudo ufw default deny incoming
sudo ufw allow 22/tcp      # SSH
sudo ufw allow 443/tcp     # HTTPS (Nginx/Caddy)
sudo ufw allow 50052/tcp   # gRPC (限制为仅 Agent IP)
sudo ufw enable

# Agent 服务器
sudo ufw default deny incoming
sudo ufw allow 22/tcp      # SSH
sudo ufw allow 443/tcp     # 代理服务端口
sudo ufw allow from HUB_IP to any port 50052  # 仅允许 Hub 连接
sudo ufw enable
```

### 3. TLS 配置

#### HTTP API TLS

ProxyPanel Hub 的 HTTP API 默认不启用 TLS。推荐使用反向代理（Nginx/Caddy）提供 TLS 终止：

```bash
# Caddy（自动证书）
caddy reverse-proxy --from panel.example.com --to localhost:8080

# Certbot + Nginx
sudo certbot --nginx -d panel.example.com -d grpc.example.com
```

#### gRPC TLS

Hub 支持原生 gRPC TLS，通过启动参数配置：

```bash
proxy-panel-hub \
  --grpc-tls-cert /path/to/server.crt \
  --grpc-tls-key /path/to/server.key
```

Agent 连接 TLS Hub：

```bash
proxy-panel-agent \
  --hub-url https://grpc.example.com:50052 \
  --tls-ca /path/to/ca.crt \
  --tls-domain grpc.example.com
```

或通过环境变量：

```bash
PROXYPANEL_GRPC_TLS_CERT=/path/to/server.crt
PROXYPANEL_GRPC_TLS_KEY=/path/to/server.key
```

### 4. Agent Token 轮换

```bash
# 生成新 Token
cargo run --bin proxy-panel -- agent-token --node-name "tokyo-01"

# 在 Hub 数据库中更新 Token（通过 API 或直接 SQL）
# 重启 Agent 使用新 Token
sudo systemctl restart proxypanel-agent
```

---

## 监控与日志

### 使用 Prometheus + Grafana

Hub 内置 Prometheus metrics 端点，无需额外配置：

```bash
# 抓取配置
scrape_configs:
  - job_name: 'proxypanel-hub'
    static_configs:
      - targets: ['localhost:8081']
    metrics_path: '/metrics'
```

访问 `http://localhost:8081/metrics` 查看所有可用指标。

### 关键指标

| 指标 | 类型 | 说明 |
|------|------|------|
| `proxypanel_http_requests_total` | Counter | HTTP 请求总数（按 method, path, status） |
| `proxypanel_agent_connections_total` | Counter | Agent 连接事件总数 |
| `proxypanel_grpc_messages_total` | Counter | gRPC 消息总数（按 type） |
| `proxypanel_active_agents` | Gauge | 当前活跃 Agent 数量 |
| `proxypanel_active_clients` | Gauge | 当前活跃客户端数量 |
| `proxypanel_active_nodes` | Gauge | 当前活跃节点数量 |

### 日志收集

```bash
# 使用 journald 收集
sudo journalctl -u proxypanel-hub -u proxypanel-agent --since "1 hour ago"

# 或使用 Vector / Fluentd 转发到 Loki / ELK
```

### 告警规则示例

```yaml
# Prometheus rules
groups:
  - name: proxypanel
    rules:
      - alert: AgentDisconnected
        expr: proxypanel_agents_connected < expected_agents
        for: 5m
        annotations:
          summary: "Agent 断开连接"
      
      - alert: HighTrafficUsage
        expr: rate(proxypanel_traffic_bytes_total[1h]) > threshold
        for: 10m
        annotations:
          summary: "流量使用异常"
```

---

## 备份与恢复

### 数据库备份

```bash
# 自动备份脚本
#!/bin/bash
BACKUP_DIR="/backup/proxypanel"
DATE=$(date +%Y%m%d_%H%M%S)
mkdir -p "$BACKUP_DIR"

pg_dump -h localhost -U proxypanel proxypanel | gzip > "$BACKUP_DIR/proxypanel_$DATE.sql.gz"

# 保留最近 7 天
find "$BACKUP_DIR" -name "proxypanel_*.sql.gz" -mtime +7 -delete
```

### 配置文件备份

```bash
tar czf /backup/proxypanel/config_$(date +%Y%m%d).tar.gz /opt/proxypanel/config/
```

### 恢复

```bash
# 恢复数据库
gunzip < proxypanel_20240115_120000.sql.gz | psql -h localhost -U proxypanel proxypanel

# 恢复配置
tar xzf config_20240115.tar.gz -C /
```

---

## 升级指南

### Docker Compose 升级

```bash
cd /opt/proxypanel

# 拉取最新代码
git pull origin main

# 重新构建
docker compose down
docker compose up -d --build

# 运行迁移
docker compose exec hub proxy-panel init-db --database-url "$PROXYPANEL_DATABASE_URL"
```

### 手动升级

```bash
# 1. 下载新版本
cd /opt/proxypanel
git pull origin main

# 2. 编译
cargo build --release --workspace

# 3. 备份旧版本
sudo mv /opt/proxypanel/bin/proxy-panel-hub /opt/proxypanel/bin/proxy-panel-hub.bak
sudo cp target/release/proxy-panel-hub /opt/proxypanel/bin/

# 4. 运行数据库迁移
/opt/proxypanel/bin/proxy-panel init-db --database-url "..."

# 5. 重启服务
sudo systemctl restart proxypanel-hub
sudo systemctl restart proxypanel-agent

# 6. 验证
sudo systemctl status proxypanel-hub

# 7. 清理旧版本（确认正常后）
sudo rm /opt/proxypanel/bin/proxy-panel-hub.bak
```

---

## CI/CD 产物说明

项目通过 GitHub Actions 自动化构建与发布：

### CI Workflow (`.github/workflows/ci.yml`)

| Job | 触发条件 | 说明 |
|-----|----------|------|
| `rust` | push 到 `main`/`master`、任意 PR | 执行 `cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace` |
| `web` | 同上 | 在 `crates/pp-web` 执行 `bun run verify`（构建 + Linter + 格式检查） |

### Release Workflow (`.github/workflows/release.yml`)

| Job | 触发条件 | 说明 |
|-----|----------|------|
| `web` | push `v*` 标签、手动触发 | 构建前端产物并上传 artifact |
| `build` | `web` 完成后 | 在 `ubuntu-22.04` (x86_64) 与 `ubuntu-24.04-arm` (aarch64) 上交叉编译 Release 二进制 |
| `release` | `build` 完成后 | 收集 tar.gz、生成 `SHA256SUMS`、创建 GitHub Release（自动识别 prerelease） |
| `docker` | `build` 完成后 | 构建并推送 GHCR 镜像 `ghcr.io/ybakiame/proxy-panel-hub` 与 `ghcr.io/ybakiame/proxy-panel-agent` |

### 产物命名

| 产物 | 文件名 |
|------|--------|
| Hub 二进制包 (x86_64) | `proxy-panel-hub-linux-x86_64.tar.gz`（含前端 dist） |
| Hub 二进制包 (aarch64) | `proxy-panel-hub-linux-aarch64.tar.gz`（不含前端 dist） |
| Agent 二进制包 | `proxy-panel-agent-linux-{x86_64,aarch64}.tar.gz` |
| 校验文件 | `SHA256SUMS` |
| Hub 容器镜像 | `ghcr.io/ybakiame/proxy-panel-hub:latest` / `:vX.Y.Z` |
| Agent 容器镜像 | `ghcr.io/ybakiame/proxy-panel-agent:latest` / `:vX.Y.Z` |

---

## 故障排查

### Hub 无法启动

```bash
# 检查数据库连接
/opt/proxypanel/bin/proxy-panel diagnose --database-url "..."

# 检查端口占用
sudo ss -tlnp | grep -E '8080|8081|50052'

# 查看详细日志
sudo journalctl -u proxypanel-hub -n 100 --no-pager
```

### Agent 无法连接 Hub

```bash
# 测试网络连通性
curl -v http://HUB_IP:50052

# 检查 Token
cat /var/lib/proxypanel/.agent_token

# 查看 Agent 日志
sudo journalctl -u proxypanel-agent -f

# 在 Hub 侧检查 Agent 是否被接受
sudo journalctl -u proxypanel-hub | grep "agent"
```

### 数据库连接问题

```bash
# 测试 PostgreSQL
psql -h localhost -U proxypanel -d proxypanel -c "SELECT 1"

# 检查连接数
psql -c "SELECT count(*) FROM pg_stat_activity;"

# 重启 PostgreSQL
sudo systemctl restart postgresql
```

### 性能问题

```bash
# 检查 Hub CPU / 内存
top -p $(pgrep proxy-panel-hub)

# 检查数据库慢查询
psql -c "SELECT query, calls, mean_time FROM pg_stat_statements ORDER BY mean_time DESC LIMIT 10;"

# 检查 gRPC 连接数
sudo ss -tn | grep 50052 | wc -l
```

---

## 参考

- [systemd 服务文档](https://www.freedesktop.org/software/systemd/man/systemd.service.html)
- [PostgreSQL 高可用](https://www.postgresql.org/docs/current/high-availability.html)
- [Caddy 文档](https://caddyserver.com/docs/)
- [Nginx gRPC 代理](https://www.nginx.com/blog/nginx-1-13-10-grpc/)
