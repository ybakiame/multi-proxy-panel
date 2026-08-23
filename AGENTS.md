# AGENTS.md — ProxyPanel 开发代理指南

本文件面向 AI 编码代理和自动化工具，描述项目的构建方式、代码规范、关键约定和常见任务。

---

## 1. 项目概述

ProxyPanel 是 Rust Workspace 项目，采用 **Hub-Agent** 架构：

- **Hub** (`pp-hub`): 中央管理面板，暴露 HTTP REST API + gRPC 双向流服务
- **Agent** (`pp-agent`): 部署在代理节点上，管理 sing-box/mihomo 进程，通过 gRPC 长连接与 Hub 通信
- **Web** (`pp-web`): React 前端，通过 HTTP API 与 Hub 交互（Vite + TypeScript + HeroUI + Tailwind CSS）

---

## 2. 构建系统

### 2.1 工具链

- Rust 1.86+（Workspace 指定 `rust-version = "1.86"`，edition = "2024"）
- 使用 `rust-toolchain.toml` 锁定工具链
- 构建前端需要 Node.js 20+ 和 npm（`crates/pp-web` 为独立的 Vite + React 项目）

### 2.2 常用构建命令

```bash
# 编译所有 crate
cargo build --workspace

# 编译 Release
cargo build --release --workspace

# 运行 Hub
cargo run --release --bin proxy-panel-hub

# 运行 Agent
cargo run --release --bin proxy-panel-agent -- --hub-url http://localhost:50052

# 运行 CLI
cargo run --bin proxy-panel -- <COMMAND>

# 运行测试
cargo test --workspace

# 检查（严格）
cargo clippy --workspace --all-targets -- -D warnings

# 格式化
cargo fmt --all
```

### 2.3 Web 前端构建

`crates/pp-web` 使用 Bun 作为包管理器（见 `packageManager` 字段）。

```bash
cd crates/pp-web
# 安装依赖
bun install
# 开发模式
bun run dev
# 发布构建
bun run build
# 产物位于 crates/pp-web/dist/
```

前端代码检查与格式化已集成 oxc 工具链：

```bash
cd crates/pp-web
# Linter（oxlint + React / a11y / import 插件，配置在 .oxlintrc.json）
bun run lint
# 格式化（oxfmt 处理 TS/TSX）
bun run format
# 格式检查
bun run format:check
# 最终核验：构建 + Linter + 格式检查
bun run verify
```

提交前必须在 `crates/pp-web` 目录执行 `bun run verify` 并全部通过。

---

## 3. 代码规范

### 3.1 命名约定

| 项目 | 规范 | 示例 |
|------|------|------|
| Crate | `pp-{name}` | `pp-hub`, `pp-agent` |
| 二进制 | `proxy-panel{-suffix}` | `proxy-panel-hub`, `proxy-panel-agent` |
| 模块 | `snake_case` | `agent_service.rs` |
| 类型 / Trait | `PascalCase` | `AppState`, `ConfigBuilder` |
| 函数 / 变量 | `snake_case` | `list_nodes`, `push_config` |
| 常量 | `SCREAMING_SNAKE_CASE` | `MAX_RETRY_DELAY` |
| 错误类型 | `PanelError` | `PanelError::Database(...)` |
| Result 别名 | `PanelResult<T>` | `PanelResult<Value>` |

### 3.2 模块组织

每个 crate 的 `src/lib.rs` 或 `src/main.rs` 顶部的模块声明顺序：

1. `mod` 声明（按字母序）
2. `use` 引入（标准库 → 外部 crate → 内部 crate → 本 crate）
3. `pub use` 重导出

示例：

```rust
//! Crate-level documentation (required).

pub mod crypto;
pub mod error;
pub mod models;

pub use crypto::*;
pub use error::*;
pub use models::*;
```

### 3.3 错误处理

- 所有 crate 统一使用 `PanelError` / `PanelResult<T>`（定义于 `pp-common`）
- gRPC handler 内部可使用 `anyhow::Result` 简化传播
- HTTP handler 使用 `Result<T, StatusCode>`，将业务错误映射为 HTTP 状态码
- 禁止裸 `unwrap()` / `expect()`，仅在测试或 `main` 的初始化阶段允许

### 3.4 异步规范

- 统一使用 `tokio` 运行时
- Sea-ORM 操作必须 `.await`
- gRPC stream 使用 `tokio::sync::mpsc` 通道
- 共享状态使用 `Arc<AppState>` + `tokio::sync::RwLock`

### 3.5 数据库规范

- 使用 Sea-ORM Migration 管理 schema，禁止手写 SQL 修改生产库
- 实体定义于 `crates/pp-db/src/entities/`
- 主键统一使用应用层生成的 `Uuid v4`
- JSON 字段使用 Sea-ORM 的 `Json` 类型
- 时间戳使用 `timestamp_with_time_zone`
- **数据迁移走版本化升级**：schema 变更用 Migration；逻辑/数据迁移（如清理废弃功能数据）注册到 `crates/pp-db/src/upgrade.rs` 的 `UPGRADE_STEPS`，按 `introduced_in` 版本门控执行一次（存于 `system_meta.app_version`），不要写成每次启动都执行

### 3.6 Protobuf / gRPC

- `.proto` 文件位于 `proto/hub_agent.proto`
- 生成代码位于 `crates/pp-proto/src/lib.rs`（由 `build.rs` 自动生成）
- **禁止直接修改** `crates/pp-proto/src/lib.rs`，应修改 `.proto` 后重新构建
- proto 中的枚举值与 Rust 枚举通过显式 `match` 转换（参见 `pp-agent/src/client.rs` 的 `core_type_from_i32`）

---

## 4. 关键架构决策

### 4.1 Hub-State 设计

`AppState` 是 Hub 的核心共享状态：

```rust
pub struct AppState {
    pub db: DatabaseConnection,
    pub agents: Arc<RwLock<HashMap<Uuid, AgentConnection>>>,
}
```

- `db`: Sea-ORM 数据库连接
- `agents`: 内存中的 Agent 连接表，用于向指定节点推送消息

**注意**：`AppState` 使用 `Arc<AppState>` 传递，本身已实现 `Clone`（浅拷贝）。

### 4.2 Agent 重连机制

Agent 的 gRPC 客户端实现了指数退避重连：

- 初始延迟：1 秒
- 最大延迟：60 秒
- 重连因子：×2

重连注册时 Agent 上报各核心已应用的 `config_version`（来自本地 `last_config.<core>.json` 快照），Hub 调度推送前比对版本，一致则不推送，避免重连后不必要的核心重启。

### 4.2.1 核心版本管理（active 制 + 手动灰度推送）

`core_versions` 表同时承载版本目录与在用版本：每核心类型一行 `is_active`，`effective_core_version` 读取 active 行（无 active 时 sing-box 回退 v1.13.14、mihomo 回退最新 release）。`POST /api/v1/core-versions/{id}/activate` 切换 active，仅打标不推送。

**pending 脏标记模型**：协议/绑定/中继/核心版本变更不再自动推送，而是写入 `node_pending_updates`（node_id + core_type + update_type=config|core）。`GET /api/v1/nodes/pending-updates` 观测灰度状态，`POST /api/v1/nodes/push-pending`（可按 node_ids/core_type 过滤）手动批量推送，成功后消标；单节点 `POST /nodes/{id}/push` 同样消标。流量超限/到期的强制下线推送仍保持自动（访问控制而非发布管理）。

Agent 侧按 `.build_id.<core>` 标记（`version|build_id`）判定核心二进制是否需要重装：版本或上游构建任一变化即"先备份后还原"式重下。

### 4.2.2 统一证书管理（Agent 内置 ACME）

`certificates` 表管理托管证书。Agent 内置 instant-acme 客户端，通过临时 80 端口监听完成 HTTP-01 挑战，证书统一落盘 `<data_dir>/certs/<domain>.{crt,key}`。

TLS 为分层模型：协议配置仅以 `tls_settings.enabled` 声明是否启用 TLS，具体证书在节点绑定的 `override_settings.tls_settings` 复写（`cert_id` 托管证书 / `certFile+keyFile` 显式文件 / `domain` 内置 ACME 仅 sing-box）。Hub 生成配置时把 `cert_id` 翻译为 `managed_domain` 路径（三个核心统一为 `certs/<domain>.{crt,key}`），订阅链接 SNI 取证书域名。Agent 每日检查，证书满 60 天自动续期并 reload 引用它的核心（mihomo 靠证书文件监听自动热加载，sing-box 由 Agent 按快照 restart）。

### 4.3 配置生成流程

```
ProtocolConfig (DB)
    ↓
NodeBinding (DB: node_id + protocol_config_id)
    ↓
pp-config BuilderRegistry
    ↓
sing-box JSON / mihomo YAML（Hub↔Agent 间始终以 JSON 传输，mihomo 由 Agent 落盘时转 YAML）
    ↓
config_version = SHA-256(config_json) 前 16 位（内容哈希，未变更则 Hub 调度推送与 Agent 应用均跳过）
    ↓
HubMessage::ConfigPush (gRPC)
    ↓
Agent → CoreManager::reload() / restart()
```

### 4.3.1 服务端中继路由（relay_rules）

`relay_rules` 表定义入口节点的服务端分流：入口节点照常接入客户端，命中的域名经核心路由规则转发到出口绑定的入站（链式 outbound），其余流量直连。用于"好线路入口 + 原生 IP 出口"组合解锁。

- 匹配模式：`inline`（domain/domain_suffix 列表）或 `rule_set`（内置社区规则集 / 自定义 per-core URL；sing-box 用 remote rule_set，mihomo 用 rule-providers，格式按核心分别取自规则集库）
- 中继凭证：创建规则时自动开通 `relay-<name>` 系统客户端并复制出口绑定分组，凭证随分组注入出口入站 users；中继流量在出口节点按该客户端单独记账
- 出口协议限 vless_reality / hysteria2 / anytls；relay outbound 由 `pp-config/src/relay.rs` 按双核心分别构建，SNI 取出口绑定 TLS 复写的证书域名
- 配置注入在 `generate_node_config` 末尾（`apply_relay_rules`），规则增删改对入口与出口节点均重推配置

### 4.4 订阅生成流程

```
Subscription (token)
    ↓
查找 Client → 查找 active Bindings
    ↓
inject_client_credentials() 注入 UUID/Email/Password
    ↓
pp-subscription::generate_subscription(format)
    ↓
Base64 / JSON / Clash YAML / SingBox JSON / V2RayNG
```

---

## 5. Git 提交规范（代理必读）

### 原子化提交

每次修改代码时，**一个提交只包含一个逻辑变更单元**。这是本项目最重要的代码规范之一。

**DO:**
- 一个提交 = 一个功能修复 / 一个重构 / 一组相关文档更新
- 提交前运行 `cargo build --workspace` 和 `cargo test --workspace` 确保通过
- 使用 `git add -p`（patch 模式）精选要提交的变更行

**DON'T:**
- 不要把修复 A 文件的 Bug 和格式化 B 文件混在一个提交里
- 不要提交半成品代码（无法编译或测试失败）
- 不要提交 IDE 配置文件、临时文件等无关变更

**提交信息格式：**

```
<type>(<scope>): <subject>

<body>
```

- `type`: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`
- `scope`: `hub`, `agent`, `web`, `cli`, `db`, `config`, `core`, `sub`, `proto`, `common`, `docs`

**示例：**

```bash
git commit -m "fix(hub): 修复节点推送配置时 core_type 解析错误"
git commit -m "docs: 更新 API 参考中的订阅端点说明"
git commit -m "refactor(db): 提取流量查询为独立 service 方法"
```

---

## 6. 常见修改任务

### 5.1 添加新的数据库实体

1. 在 `crates/pp-db/src/migration/` 中创建新迁移文件
2. 在 `crates/pp-db/src/migration/mod.rs` 注册迁移
3. 运行 `cargo run --bin proxy-panel -- init-db`
4. （可选）生成实体：`sea-orm-cli generate entity -o src/entities`

### 5.2 添加新的 HTTP API 端点

1. 在 `crates/pp-hub/src/routes/` 新建或修改路由模块
2. 在 `crates/pp-hub/src/routes/mod.rs` 导出
3. 在 `crates/pp-hub/src/main.rs` 的 Router 中注册路由
4. 如需新业务逻辑，在 `crates/pp-hub/src/service/` 添加 service 方法

### 5.3 添加新的 gRPC 消息类型

1. 修改 `proto/hub_agent.proto`
2. 在 `AgentMessage` 或 `HubMessage` 的 `oneof payload` 中添加新字段
3. 重新构建 `pp-proto`：`cargo build -p pp-proto`
4. 在 `pp-hub/src/grpc/agent_service.rs` 添加处理逻辑
5. 在 `pp-agent/src/client.rs` 添加发送/接收逻辑

### 5.4 添加新的订阅格式

1. 在 `crates/pp-subscription/src/formats/` 新建模块
2. 在 `crates/pp-subscription/src/formats/mod.rs` 导出
3. 在 `crates/pp-subscription/src/generator.rs` 的 `SubscriptionFormat` 和 `generate_subscription` 中添加分支

### 5.5 添加新的协议支持

1. 在 `pp-common/src/protocol.rs` 的 `ProtocolType` 中添加变体
2. 在 `pp-config/src/singbox.rs` 和/或 `pp-config/src/mihomo.rs` 实现对应的 `build_inbound`
3. 在 `pp-hub/src/routes/protocol.rs` 的 `validate_protocol` 中添加校验规则
4. 在 `pp-subscription` 相关格式中添加节点序列化逻辑

---

## 7. 测试策略

- 单元测试：各 crate 的 `src/` 中内联 `#[cfg(test)]` 模块
- 集成测试：尚未设置，计划添加 `tests/` 目录
- 数据库测试：使用 `tokio-test` + 内存 SQLite
- gRPC 测试：使用 `tonic` 的内存通道

---

## 8. 调试技巧

### 查看 Agent 日志

```bash
RUST_LOG=proxy_panel_agent=debug,pp_core=debug cargo run --bin proxy-panel-agent
```

### 查看 Hub 日志

```bash
RUST_LOG=proxy_panel_hub=debug,tower_http=debug cargo run --bin proxy-panel-hub
```

### 测试 gRPC 连接

```bash
grpcurl -plaintext localhost:50052 list proxypanel.HubAgent
```

---

## 9. 文件变更后需同步的文档

修改以下内容时，请同步更新对应文档：

| 变更内容 | 需更新文档 |
|----------|-----------|
| 新增/修改 API 端点 | `docs/api_reference.md` |
| 新增/修改数据库实体 | `docs/architecture.md` |
| 新增 crate 或模块 | `README.md`, `docs/architecture.md` |
| 构建流程变化 | `README.md`, `docs/development.md` |
| 部署方式变化 | `docs/deployment.md` |
| 代码规范变化 | `AGENTS.md` |

---

## 10. 联系方式与资源

- 仓库: `https://github.com/ybakiame/multi-proxy-panel`
- Issues: 使用 GitHub Issues
- 文档目录: `docs/`
