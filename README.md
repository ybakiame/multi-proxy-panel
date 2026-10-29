# ProxyPanel

> 面向多节点代理服务的集中管理面板，采用 Rust 后端与 React 管理界面。

[![Rust Version](https://img.shields.io/badge/rust-1.88%2B-blue)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-AGPL--3.0-orange)](LICENSE)

## 项目简介

ProxyPanel 采用 **Hub-Agent** 架构，支持多节点统一管理、协议配置、订阅生成、流量统计与主机指标监控。用户侧客户端由独立的 [岚径 · Lanpath 仓库](https://github.com/ybakiame/lanpath) 维护，通过公开订阅/API 与本项目协作。

### 核心特性

- **节点预生成与状态监控**：在 Hub 创建节点并生成安装指令，Agent 通过心跳保持在线状态
- **安全协议配置**：VLESS + REALITY、VLESS + XHTTP、Hysteria2、AnyTLS
- **双核心支持**：[sing-box](https://github.com/SagerNet/sing-box) 与 [mihomo](https://github.com/MetaCubeX/mihomo)
- **订阅与节点导出**：生成分享链接、Clash YAML、sing-box JSON 和通用 JSON 节点数据
- **流量与指标**：按入站和用户统计流量，采集 CPU、内存、磁盘、网络与系统负载
- **版本化配置推送**：内容哈希去重，支持手动灰度推送以及按需 reload/restart
- **gRPC 双向流**：Hub 与 Agent 通过长连接进行配置下发和状态上报
- **管理界面**：React + shadcn/ui（Base UI） + Tailwind CSS
- **数据库**：PostgreSQL 与 SQLite

## 系统架构

```mermaid
flowchart TB
    subgraph hub["ProxyPanel Hub"]
        direction TB
        api["HTTP API (Axum)"]
        grpc["gRPC Stream (Tonic)"]
        web["Static Web App (React)"]
        biz["Business Layer<br/>(Services / State)"]
        db[("Database (Sea-ORM)<br/>PostgreSQL / SQLite")]
        api --> biz
        grpc --> biz
        biz --> db
        api --> web
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

本仓维护 Hub、Agent、管理 Web、CLI 与服务端共享模块，不包含 Lanpath 客户端源码或构建依赖。

## 支持的协议

| 协议 | sing-box | mihomo | 说明 |
|------|:--------:|:------:|------|
| VLESS + REALITY | ✅ | ✅ | REALITY 传输 |
| VLESS + XHTTP | ❌ | ✅ | XHTTP 传输，当前仅支持 mihomo |
| Hysteria2 | ✅ | ✅ | 基于 QUIC |
| AnyTLS | ✅ | ✅ | AnyTLS 协议 |

协议与核心的兼容关系以 `pp-common/src/protocol.rs` 和对应配置构建器为准。

## 订阅输出

| 输出 | 用途 |
|------|------|
| Base64 编码的分享链接 | 支持分享链接订阅的客户端 |
| V2RayNG 兼容输出 | 与 Base64 分享链接生成器相同，便于客户端选择 |
| Clash YAML | Clash/mihomo 配置 |
| sing-box JSON | sing-box 配置，可使用内置或自定义模板 |
| 通用 JSON 节点数组 | 调试、检查或二次处理，不是独立客户端订阅格式 |

订阅链接由 `/sub/{token}` 提供；格式是输出选项，不会改变公开订阅端点。

## 快速开始

本地构建、运行和验证统一使用仓库 [Nix 开发环境](nix-flake.md)。开发命令均在 `nix develop --no-pure-eval` 环境中执行。

```bash
git clone https://github.com/ybakiame/multi-proxy-panel.git
cd multi-proxy-panel
nix develop --no-pure-eval

# 启动开发数据库并初始化
docker compose up -d postgres
cargo run --bin proxy-panel -- init-db \
  --database-url "postgres://proxypanel:proxypanel@localhost/proxypanel"

# 构建管理界面并启动 Hub
bun install
bun run --filter pp-web build
cargo run --bin proxy-panel-hub
```

Hub 默认监听 `localhost:8081`（HTTP）和 `localhost:50052`（gRPC）。首次启动的管理凭证、前端开发模式及 Agent 调试方法见 [开发指南](docs/development.md)。

节点不采用自动注册。请先在面板 **节点管理** 中预生成节点，再将面板生成的安装指令复制到目标 Agent 服务器执行；外部 Agent 通常通过 `https://面板域名:443` 连接，而不是直接连接内部 50052 端口。

生产安装、Caddy/Nginx 反向代理、公开订阅与管理认证隔离、升级和回滚见 [部署指南](docs/deployment.md)。

## 项目结构

```text
multi-proxy-panel/
├── proto/                  # Hub-Agent 与 sing-box 守护进程协议
├── crates/
│   ├── pp-common/          # 共享类型、错误、加密与日志工具
│   ├── pp-db/              # Sea-ORM 实体、迁移与版本化数据升级
│   ├── pp-proto/           # Protobuf 生成代码
│   ├── pp-config/          # sing-box/mihomo 配置构建器
│   ├── pp-core/            # 核心进程管理
│   ├── pp-subscription/    # 分享链接、订阅配置与 JSON 节点数据生成
│   ├── pp-hub/             # HTTP API、gRPC 服务与静态文件托管
│   ├── pp-agent/           # 节点 Agent
│   └── pp-cli/             # 安装、升级、回滚与运维 CLI
├── apps/panel/             # React 管理界面
├── config/                 # Hub 配置示例
├── deploy/                 # systemd 与 Caddy 部署模板
├── docs/                   # 项目文档（索引见 docs/index.md）
└── scripts/                # 开发、安装与检查脚本
```

主要二进制：

| 组件 | 产物 |
|------|------|
| Hub | `proxy-panel-hub` |
| Agent | `proxy-panel-agent` |
| CLI | `proxy-panel` |

## 开发与验证

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
bun run verify:rust
bun run --filter pp-web verify
```

完整流程见 [开发指南](docs/development.md) 和 [贡献指南](docs/contributing.md)。

## 文档

- [文档中心](docs/index.md)
- [部署指南](docs/deployment.md)
- [API 参考](docs/api_reference.md)
- [架构说明](docs/architecture.md)
- [Nix 开发环境](nix-flake.md)
- [ADR 索引](docs/adr/README.md)
- [变更日志](CHANGELOG.md)

## 安全说明

- 节点必须在 Hub 预生成，Agent Token 以 Argon2 哈希存储在 Hub 数据库中
- 安装后 Agent Token 由权限受限的 env 文件提供，不写入 systemd 进程参数
- 生产环境应通过 Nginx/Caddy 启用 HTTPS，并将 Hub 的内部 HTTP/gRPC 监听限制在本机
- 管理 UI/API 与公开订阅、安装脚本及 Agent gRPC 应分别配置访问控制
- 数据库连接建议使用专用账号并限制权限

## 贡献与许可证

欢迎提交 Issue 和 PR，请先阅读 [贡献指南](docs/contributing.md)。

本项目采用 [AGPL-3.0-or-later](LICENSE) 许可证开源。
