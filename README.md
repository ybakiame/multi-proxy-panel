# ProxyPanel

> 一个现代化的代理节点集中管理面板，采用 Rust 全栈构建。

[![Rust Version](https://img.shields.io/badge/rust-1.88%2B-blue)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-AGPL--3.0-orange)](LICENSE)

## 项目简介

ProxyPanel 是一个开源的代理服务管理面板，采用 **Hub-Agent** 架构设计。它支持多节点统一管理、多协议配置、自动订阅生成、实时流量统计与主机监控，并提供现代化的 Web 管理界面与跨平台桌面客户端。

### 核心特性

- **多节点管理**: 支持无限节点注册，自动心跳检测与状态监控
- **多协议支持**: VLESS (REALITY / Vision / XHTTP)、VMess、Trojan、Shadowsocks 2022、Hysteria2、TUIC v5
- **多核心兼容**: 同时支持 [sing-box](https://github.com/SagerNet/sing-box) 与 [mihomo](https://github.com/MetaCubeX/mihomo)
- **订阅系统**: 自动生成 Base64、JSON、Clash、SingBox、V2RayNG 格式订阅链接
- **实时流量统计**: 按入站端口和用户维度统计上传/下载流量
- **主机监控**: CPU、内存、磁盘、网络、系统负载实时上报
- **配置热重载**: 无需重启即可向节点推送配置更新
- **gRPC 双向流**: Hub 与 Agent 之间通过长连接双向实时通信
- **桌面客户端**: 基于 Tauri 的跨平台桌面应用（Linux/Windows/macOS），内置脚本引擎与 HTTPS MITM 抓包重写（MITM 为桌面端能力，移动端不支持）
- **移动客户端**: 基于 Tauri 的 Android 客户端，核心由内置 Go 引擎驱动（VPN 模式，无 MITM）
- **脚本引擎**: 兼容 Quantumult X / Surge / Loon 三方言 API 的 JS 脚本运行时（QuickJS）
- **HTTPS 解密与重写**: URL / Header / Body 重写、Reject / Mock、请求响应脚本钩子、流量抓包（桌面端专属）
- **现代化前端**: 基于 React + HeroUI + Tailwind CSS 的响应式 Web 管理界面
- **多数据库支持**: PostgreSQL (生产) / SQLite (开发测试)

## 系统架构

```
┌─────────────────────────────────────────────────────────────────────┐
│                           ProxyPanel Hub                            │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │   HTTP API   │  │  gRPC Stream │  │   Web App    │              │
│  │   (Axum)     │  │   (Tonic)    │  │  (React)     │              │
│  └──────────────┘  └──────────────┘  └──────────────┘              │
│         │                │                  │                       │
│         └────────────────┼──────────────────┘                       │
│                          ▼                                          │
│              ┌─────────────────────┐                                │
│              │    Business Layer   │                                │
│              │  (Services / State) │                                │
│              └─────────────────────┘                                │
│                          │                                          │
│                          ▼                                          │
│              ┌─────────────────────┐                                │
│              │   Database (Sea-ORM)│                                │
│              │ PostgreSQL / SQLite │                                │
│              └─────────────────────┘                                │
└─────────────────────────────────────────────────────────────────────┘
                                    │ gRPC (双向流)
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                        ProxyPanel Agent (Node)                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │ gRPC Client  │  │   Reporter   │  │   Monitor    │              │
│  │              │  │(Traffic/Logs)│  │(Host Metrics)│              │
│  └──────────────┘  └──────────────┘  └──────────────┘              │
│         │                │                  │                       │
│         └────────────────┼──────────────────┘                       │
│                          ▼                                          │
│              ┌─────────────────────┐                                │
│              │    Core Supervisor  │                                │
│              │ sing-box / mihomo  │                                │
│              └─────────────────────┘                                │
└─────────────────────────────────────────────────────────────────────┘
```

客户端分桌面（`apps/desktop`，`pp-client-ui`）与移动（`apps/mobile`，`pp-client-mobile-ui`）两个独立 Tauri 应用，共享前端库 `@pp/client-core` 与 Rust 命令层 `pp-client-tauri`。桌面客户端运行在用户设备上，经由订阅端点从 Hub 拉取节点配置，在本地驱动 sing-box 核心（Clash 格式订阅经节点转换后同样由 sing-box 运行），并叠加 MITM 与脚本引擎实现 HTTPS 解密与抓包重写（MITM 为桌面端能力，移动端不支持）；移动客户端由内置 Go 引擎（`panel-core` → `panelcore.aar`）驱动核心。桌面客户端链路：

```
┌─────────────────────────────────────────────────────────────────────┐
│                      ProxyPanel Client (Desktop)                     │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │ pp-script    │  │ pp-mitm      │  │ pp-core      │              │
│  │ 脚本引擎     │  │ MITM 引擎    │  │ 核心子进程    │              │
│  └──────────────┘  └──────────────┘  └──────────────┘              │
│         │                │                  │                       │
│         └────────────────┼──────────────────┘                       │
│                          ▼                                          │
│              ┌─────────────────────────────┐                        │
│              │    pp-client (ClientState)   │                        │
│              │  订阅同步 / 配置合成 / 系统代理 │                        │
│              └─────────────────────────────┘                        │
└─────────────────────────────────────────────────────────────────────┘
            │ 订阅 (HTTP)                           │ 本地代理流量
            ▼                                        ▼
   Hub /sub/{token} 公开订阅端点            远端代理节点 (sing-box / mihomo)
```

## 快速开始

### 环境要求

- Rust 1.88+（参见 `rust-toolchain.toml`）
- PostgreSQL 15+ (或 SQLite 用于开发)
- Bun 1.3+ (构建 Web 前端，见 `apps/panel/package.json` 的 packageManager 字段)


### 1. 克隆项目

```bash
git clone https://github.com/ybakiame/multi-proxy-panel.git
cd proxy-panel
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

```bash
cd apps/panel
bun install
bun run build
```

产物位于 `apps/panel/dist/`，Hub 会自动从该目录托管静态文件（可通过 `--static-dir` 覆盖）。

开发模式热重载：

```bash
cd apps/panel
bun run dev
```

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

### 9. 构建桌面客户端（可选）

桌面客户端为独立的 Tauri 项目（退出根 workspace），使用 Bun 作为包管理器：

```bash
cd apps/desktop
bun install
bun run tauri dev      # 开发模式（Vite 热重载 + Tauri 窗口）
bun run tauri build    # 发布构建（产物位于 src-tauri/target/release/）
```

#### Android（移动客户端）构建

Android 客户端自 desktop 拆分独立为 `apps/mobile`（Tauri 2 移动应用）。构建涉及 NDK 交叉编译、Go 核心 `panel-core` 的 AAR 打包与 GEO 数据准备，完整步骤见 [docs/development.md](docs/development.md) 的「[Android 客户端构建](docs/development.md#android-客户端构建)」章节。

## 项目结构

```
proxy-panel/
├── Cargo.toml              # Workspace 根配置
├── docker-compose.yml      # 开发环境编排
├── proto/
│   └── hub_agent.proto     # Hub-Agent gRPC 协议定义
├── crates/
│   ├── pp-common/          # 共享类型、错误、工具函数
│   ├── pp-db/              # Sea-ORM 实体与迁移
│   ├── pp-proto/           # gRPC Protobuf 生成代码
│   ├── pp-config/          # sing-box/mihomo 配置构建器
│   ├── pp-core/            # 核心进程管理抽象
│   ├── pp-subscription/    # 订阅链接生成器
│   ├── pp-script/          # 客户端脚本引擎（QuickJS + QX/Surge/Loon 方言）
│   ├── pp-mitm/            # HTTPS MITM 引擎（hudsucker 封装 + 重写/抓包，桌面专属）
│   ├── pp-client/          # 客户端核心库（订阅/配置合成/系统代理）
│   ├── pp-client-tauri/    # 双端共享 Tauri 命令层（state/logs/capabilities/命令 + android core_bridge）
│   ├── pp-hub/             # 中央管理面板 (HTTP + gRPC)
│   ├── pp-agent/           # 节点代理程序
│   └── pp-cli/             # 管理 CLI 工具
├── apps/
│   ├── panel/              # 管理系统：React Web 前端（Vite + HeroUI + Tailwind）
│   ├── desktop/            # 客户端：Tauri 2 桌面应用（Linux/Windows/macOS，React 前端 + 独立 cargo 项目）
│   └── mobile/             # 客户端：Tauri 2 移动应用（Android）+ 安卓核心 Go 模块（panel-core）与构建脚本
├── packages/
│   └── client-core/        # @pp/client-core：desktop/mobile 共享前端库（api/hooks/atoms/工具）
├── docs/                   # 项目文档
└── scripts/                # 辅助脚本
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
| `pp-core` | 核心进程管理：启动、停止、重载、流量采集 | 库 |
| `pp-subscription` | 订阅生成：Base64、Clash、SingBox、V2RayNG 等格式 | 库 |
| `pp-script` | 客户端 JS 脚本引擎：rquickjs 后端 + QX/Surge/Loon 方言 API 适配与 cron 调度 | 库 |
| `pp-mitm` | HTTPS MITM 引擎（桌面端专属，移动端不支持）：CA 管理、hudsucker 封装、URL/Header/Body 重写、脚本钩子、抓包、上游代理 | 库 |
| `pp-client` | 客户端核心库：订阅同步、核心配置合成（含 MITM 链路，桌面端专属）、系统代理、生命周期编排 | 库 |
| `pp-client-tauri` | 双端共享 Tauri 命令层：state / logs / capabilities / 35 条通用命令单份实现，Android 专属 core_bridge（cfg 门控） | 库 |
| `@pp/client-core` | desktop/mobile 共享前端库：api 的 invoke 封装 + hooks + atoms + 纯工具（bun workspaces 成员） | 库 |
| `apps/desktop` | Tauri 2 桌面客户端（React 19 + Vite 8 + HeroUI，bun workspaces 成员；壳为退出根 workspace 的独立 cargo 项目） | 桌面应用 |
| `apps/mobile` | Tauri 2 移动客户端（Android：移动 UI + 壳 + Go 核心 `panel-core`，bun workspaces 成员） | Android 应用 |

## 支持的协议

| 协议 | sing-box | mihomo | 说明 |
|------|:--------:|:------:|------|
| VLESS + REALITY | ✅ | ✅ | 推荐，最低特征 |
| VLESS + XHTTP | ❌ | ✅ | 新一代传输 |
| Hysteria2 | ✅ | ✅ | QUIC 加速 |
| AnyTLS | ✅ | ✅ | 新兴协议 |

## 开发指南

详见 [docs/development.md](docs/development.md)。

快速命令：

```bash
# 运行测试
cargo test --workspace

# 检查代码
cargo clippy --workspace --all-targets -- -D warnings

# 格式化代码
cargo fmt --all

# 构建前端
cd apps/panel && bun install && bun run build

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

欢迎提交 Issue 和 PR！请阅读 [docs/contributing.md](docs/contributing.md) 了解详情。

## 许可证

本项目采用 [AGPL-3.0-or-later](LICENSE) 许可证开源。

---

<p align="center">Made with ❤️ by ProxyPanel Contributors</p>
