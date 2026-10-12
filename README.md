# ProxyPanel

> 一个现代化的代理节点集中管理面板，采用 Rust 全栈构建。

[![Rust Version](https://img.shields.io/badge/rust-1.88%2B-blue)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-AGPL--3.0-orange)](LICENSE)

## 项目简介

ProxyPanel 是一个开源的代理服务管理面板，采用 **Hub-Agent** 架构设计。它支持多节点统一管理、多协议配置、自动订阅生成、实时流量统计与主机监控，并提供现代化的 Web 管理界面。

### 核心特性

- **多节点管理**: 支持无限节点注册，自动心跳检测与状态监控
- **多协议支持**: VLESS (REALITY / Vision / XHTTP)、VMess、Trojan、Shadowsocks 2022、Hysteria2、TUIC v5
- **多核心兼容**: 同时支持 [sing-box](https://github.com/SagerNet/sing-box) 与 [mihomo](https://github.com/MetaCubeX/mihomo)
- **订阅系统**: 自动生成 Base64、JSON、Clash、SingBox、V2RayNG 格式订阅链接
- **实时流量统计**: 按入站端口和用户维度统计上传/下载流量
- **主机监控**: CPU、内存、磁盘、网络、系统负载实时上报
- **配置热重载**: 无需重启即可向节点推送配置更新
- **gRPC 双向流**: Hub 与 Agent 之间通过长连接双向实时通信
- **现代化前端**: 基于 React + HeroUI + Tailwind CSS 的响应式 Web 管理界面
- **多数据库支持**: PostgreSQL (生产) / SQLite (开发测试)

## 系统架构

```mermaid
flowchart TB
    subgraph hub["ProxyPanel Hub"]
        direction TB
        api["HTTP API (Axum)"]
        grpc["gRPC Stream (Tonic)"]
        web["Web App (React)"]
        biz["Business Layer<br/>(Services / State)"]
        db[("Database (Sea-ORM)<br/>PostgreSQL / SQLite")]
        api --> biz
        grpc --> biz
        web --> biz
        biz --> db
    end
    subgraph agent["ProxyPanel Agent (Node)"]
        direction TB
        gc["gRPC Client"]
        rep["Reporter<br/>(Traffic / Logs)"]
        mon["Monitor<br/>(Host Metrics)"]
        sup["Core Supervisor<br/>sing-box / mihomo"]
        gc --> sup
        rep --> sup
        mon --> sup
    end
    hub -- "gRPC（双向流）" --> agent
```

用户侧客户端已迁入独立的 [岚径 · Lanpath 仓库](https://github.com/ybakiame/lanpath)，
通过公开订阅/API 与 ProxyPanel 协作。两仓构建、版本和发布独立，
本仓仅维护 Hub、Agent、管理 Web、CLI 与服务端共享模块。

## 快速开始

### 环境要求

本地构建、运行和验证统一使用仓库 [Nix 开发环境](nix-flake.md)（`flake.nix` / `flake.lock`）。
以下开发命令均在 `nix develop` 内执行，Bun、Protobuf 与构建系统库使用 flake
提供的版本；Rust 经该环境的 rustup 按 `rust-toolchain.toml` 管理。

- Rust 1.88+（参见 `rust-toolchain.toml`）
- PostgreSQL 15+ (或 SQLite 用于开发)
- Bun 1.3+ (构建 Web 前端，见 `apps/panel/package.json` 的 packageManager 字段)


### 1. 克隆项目

```bash
git clone https://github.com/ybakiame/multi-proxy-panel.git
cd multi-proxy-panel
nix develop
```

### 2. 启动数据库

```bash
docker compose up -d postgres
```

### 3. 初始化数据库

```bash
cargo run --bin proxy-panel -- init-db \
  --database-url "postgres://proxypanel:proxypanel@localhost/proxypanel"
```

### 4. 创建首个管理员

```bash
cargo run --bin proxy-panel -- create-user \
  --database-url "postgres://proxypanel:proxypanel@localhost/proxypanel" \
  --username admin \
  --password "STRONG_PASSWORD"
```

### 5. 启动 Hub

```bash
cargo run --release --bin proxy-panel-hub
```

Hub 将监听：
- HTTP API: `http://localhost:8081`
- gRPC: `http://localhost:50052`

### 6. 构建 Web 前端

前端依赖在**仓库根目录**统一安装（Bun workspaces，单一 `bun.lock`）：

```bash
bun install

# panel 发布构建（产物 apps/panel/dist/）
bun run --filter pp-web build
```

产物位于 `apps/panel/dist/`，Hub 会自动从该目录托管静态文件（可通过 `--static-dir` 覆盖）。

开发模式热重载：

```bash
bun run --filter pp-web dev
```

也可 `cd apps/panel && bun run dev`（依赖仍由根目录 `bun install` 提供）。

### 7. 使用 CLI 安装 Agent（推荐）

在节点服务器上先安装 `proxy-panel` CLI，再用 CLI 安装 Agent：

```bash
# 方式一：通过 Hub 提供的 bootstrap 脚本一键安装
curl -fsSL https://<your-hub-domain>/install.sh | bash -s -- \
  --hub-url "https://grpc.example.com:50052" \
  --token "your-agent-token" \
  --name "node-01"

# 方式二：手动下载 CLI 后执行安装
# 下载对应架构的 proxy-panel-cli-linux-{x86_64,aarch64}.tar.gz 并解压到 /usr/local/bin/
sudo proxy-panel install agent \
  --hub-url "https://grpc.example.com:50052" \
  --token "your-agent-token" \
  --name "node-01"
```

安装脚本会自动完成：
- 架构检测（x86_64 / aarch64）与 glibc 兼容性检查
- 从 GitHub Release 下载 CLI tar.gz 并校验 SHA256
- 安装 CLI 到 `/usr/local/bin/`
- 调用 `proxy-panel install agent` 完成 Agent 安装、systemd 服务创建并启动

> 也可在 Web 面板的 **节点管理** 页面点击「安装指令」按钮，直接获取包含 token 的完整安装命令。

### 8. 手动启动 Agent（开发调试）

```bash
cargo run --release --bin proxy-panel-agent \
  --hub-url "http://your-hub:50052" \
  --token "your-agent-token"
```

## 项目结构

```
multi-proxy-panel/
├── Cargo.toml              # Workspace 根配置
├── docker-compose.yml      # 开发环境编排
├── proto/
│   ├── hub_agent.proto         # Hub-Agent gRPC 协议定义
│   └── singbox_daemon.proto    # sing-box 守护进程协议定义
├── crates/
│   ├── pp-common/          # 共享类型、错误、工具函数
│   ├── pp-db/              # Sea-ORM 实体与迁移
│   ├── pp-proto/           # gRPC Protobuf 生成代码
│   ├── pp-config/          # sing-box/mihomo 配置构建器
│   ├── pp-core/            # 核心进程管理抽象
│   ├── pp-subscription/    # 订阅链接生成器
│   ├── pp-hub/             # 中央管理面板 (HTTP + gRPC)
│   ├── pp-agent/           # 节点代理程序
│   └── pp-cli/             # 管理 CLI 工具
├── apps/
│   └── panel/              # 管理系统：React Web 前端（Vite + HeroUI + Tailwind）
├── docs/                   # 项目文档（索引见 docs/index.md）
├── scripts/                # 辅助脚本
└── CHANGELOG.md            # 变更日志
```

## 主要 Crate 说明

| Crate | 说明 | 产物 |
|-------|------|------|
| `pp-hub` | 中央管理面板，提供 REST API、gRPC 服务和静态文件托管 | `proxy-panel-hub` |
| `pp-agent` | 节点代理，管理本地 sing-box/mihomo 进程，上报指标 | `proxy-panel-agent` |
| `apps/panel` | 管理系统 Web 前端（bun workspaces 成员） | 静态文件 |
| `pp-cli` | 管理命令行工具：数据库初始化、Token 生成、组件生命周期管理（安装/升级/回滚/卸载/状态/日志/重启） | `proxy-panel` |
| `pp-common` | 共享模块：DTO、枚举、错误类型、加密工具 | 库 |
| `pp-db` | 数据库层：连接池、Sea-ORM 实体、迁移 | 库 |
| `pp-proto` | gRPC 协议编译生成的 Rust 代码 | 库 |
| `pp-config` | 配置抽象：将通用协议配置转译为 sing-box JSON 或 mihomo YAML | 库 |
| `pp-core` | 核心进程管理：启动、停止、重载 | 库 |
| `pp-subscription` | 订阅生成：Base64、Clash、SingBox、V2RayNG 等格式 | 库 |

## 支持的协议

| 协议 | sing-box | mihomo | 说明 |
|------|:--------:|:------:|------|
| VLESS + REALITY | ✅ | ✅ | 推荐，最低特征 |
| VLESS + XHTTP | ❌ | ✅ | 新一代传输 |
| Hysteria2 | ✅ | ✅ | QUIC 加速 |
| AnyTLS | ✅ | ✅ | 新兴协议 |

## 开发指南

完整文档索引见 [docs/index.md](docs/index.md)；开发流程、变更工作流与测试细节见 [docs/development.md](docs/development.md)。

快速命令：

```bash
# 运行测试
cargo test --workspace

# 检查代码
cargo clippy --workspace --all-targets -- -D warnings

# 格式化代码
cargo fmt --all

# Rust 全量门禁（fmt + clippy + test）
bun run verify:rust

# 前端：根目录安装依赖后按包构建/校验
bun install
bun run --filter pp-web build

# 生成实体（修改迁移后）
cd crates/pp-db && sea-orm-cli generate entity -o src/entities
```

## 部署指南

详见 [docs/deployment.md](docs/deployment.md)。

### Docker 部署（推荐）

```bash
# 启动完整栈
docker compose up -d

# 仅启动数据库
docker compose up -d postgres
```

### 使用 CLI 安装组件（推荐）

项目通过 GitHub Actions 自动构建并发布二进制文件与容器镜像：

- **GitHub Release**: 推送 `v*` 标签时自动触发，发布 `proxy-panel-cli-linux-{x86_64,aarch64}.tar.gz`、`proxy-panel-hub-linux-{x86_64,aarch64}.tar.gz` 与 `proxy-panel-agent-linux-{x86_64,aarch64}.tar.gz`，附带 `SHA256SUMS`
- **GHCR 镜像**: 同时构建并推送 `ghcr.io/ybakiame/proxy-panel-hub` 与 `ghcr.io/ybakiame/proxy-panel-agent`

**安装 Hub：**

```bash
# 通过 bootstrap 脚本
curl -fsSL https://raw.githubusercontent.com/ybakiame/multi-proxy-panel/main/scripts/install-hub.sh | bash -s --

# 或手动下载 CLI 后执行
sudo proxy-panel install hub
```

安装完成后按提示编辑 `/etc/proxy-panel/hub.toml` 配置数据库，然后：

```bash
sudo proxy-panel init-db --database-url "postgres://..."
sudo systemctl enable --now proxy-panel-hub
```

**安装 Agent：**

```bash
# 通过 bootstrap 脚本
curl -fsSL https://<your-hub-domain>/install.sh | bash -s -- \
  --hub-url "https://grpc.example.com:50052" \
  --token "your-agent-token"

# 或手动下载 CLI 后执行
sudo proxy-panel install agent \
  --hub-url "https://grpc.example.com:50052" \
  --token "your-agent-token"
```

**常用 CLI 命令速查：**

```bash
# 查看各组件状态
sudo proxy-panel status

# 升级组件
sudo proxy-panel upgrade hub
sudo proxy-panel upgrade agent
sudo proxy-panel upgrade cli

# 回滚到上一版本
sudo proxy-panel rollback hub
sudo proxy-panel rollback agent

# 查看日志
sudo proxy-panel logs hub --lines 100 --follow
sudo proxy-panel logs agent --lines 50

# 重启服务
sudo proxy-panel restart hub
sudo proxy-panel restart agent

# 卸载
sudo proxy-panel uninstall agent --purge
sudo proxy-panel uninstall hub --purge
```

详见 [docs/deployment.md](docs/deployment.md)。

### 手动部署

1. 编译 Release 版本：`cargo build --release`
2. 配置 Hub 环境变量
3. 配置 Agent 启动参数
4. 使用 systemd 管理进程（参见 docs/deployment.md）

## API 概览

Hub 提供完整的 REST API，详见 [docs/api_reference.md](docs/api_reference.md)。

主要端点：

| 端点 | 方法 | 说明 |
|------|------|------|
| `/health` | GET | 健康检查 |
| `/api/v1/nodes` | GET/POST | 节点列表 / 创建节点 |
| `/api/v1/nodes/{id}/push` | POST | 向节点推送配置 |
| `/api/v1/protocols` | GET/POST | 协议配置管理 |
| `/api/v1/clients` | GET/POST | 客户端管理 |
| `/api/v1/bindings` | GET/POST | 节点-配置绑定 |
| `/api/v1/subscriptions` | GET/POST | 订阅管理 |
| `/sub/{token}` | GET | 公开订阅端点 |
| `/api/v1/traffic` | GET | 流量查询 |
| `/api/v1/metrics` | GET | 指标查询 |
| `/api/v1/logs` | GET | 日志查询 |

## 安全说明

- Agent Token 采用加密安全的随机生成（32 bytes Base64）
- REALITY 密钥对通过 X25519 生成
- 建议在生产环境使用反向代理（Nginx / Caddy）并启用 TLS
- 数据库连接建议使用专用账号并限制权限

## 贡献指南

欢迎提交 Issue 和 PR！请阅读 [docs/contributing.md](docs/contributing.md) 了解详情；变更记录见 [CHANGELOG.md](CHANGELOG.md)。

## 许可证

本项目采用 [AGPL-3.0-or-later](LICENSE) 许可证开源。

---

<p align="center">Made with ❤️ by ProxyPanel Contributors</p>
