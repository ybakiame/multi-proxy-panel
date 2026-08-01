# ProxyPanel 架构文档

本文档描述 ProxyPanel 的整体架构、组件交互、数据流和关键技术决策。

---

## 目录

1. [整体架构](#整体架构)
2. [组件详解](#组件详解)
3. [数据流](#数据流)
4. [客户端架构](#客户端架构)
5. [数据库设计](#数据库设计)
6. [通信协议](#通信协议)
7. [安全模型](#安全模型)
8. [扩展点](#扩展点)
9. [性能考量](#性能考量)

---

## 整体架构

ProxyPanel 采用经典的 **Hub-Agent** 分布式架构，并在此基础上扩展了桌面客户端（**Client**）端。系统由四个主要部分组成：用户层（Web 管理界面、订阅客户端、桌面客户端）、Hub、Agent 与 ProxyPanel Client。

```
┌─────────────────────────────────────────────────────────────┐
│                         用户层                               │
│    Web 浏览器 ────────── 订阅客户端 (Clash/V2RayNG/...)      │
│    ProxyPanel 桌面客户端 (pp-client-ui / Tauri)              │
└─────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┴───────────────┐
              ▼                               ▼
┌─────────────────────────┐      ┌─────────────────────────┐
│     ProxyPanel Hub      │      │   公开订阅端点           │
│   (HTTP API + gRPC)     │      │   /sub/{token}          │
└─────────────────────────┘      └─────────────────────────┘
              │                                 ▲
              │ gRPC 双向流 (长连接)              │ 订阅 (HTTP)
              ▼                                 │
┌─────────────────────────────────────────────────────────────┐
│                    ProxyPanel Agent × N                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │  mihomo      │  │  sing-box    │  │  System Info │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
└─────────────────────────────────────────────────────────────┘
```

桌面客户端（详见 [客户端架构](#客户端架构)）经由 `Hub /sub/{token}` 订阅端点拉取节点配置，在本地驱动 sing-box / mihomo 核心并叠加 MITM 与脚本引擎，代理流量直连远端节点。

### 设计原则

- **无状态 Hub**: Hub 不保存运行时状态（除 Agent 连接句柄外），所有持久化数据存入数据库
- **自治 Agent**: Agent 在断连后可独立运行，重连后自动同步状态
- **配置即代码**: 协议配置存储为结构化 JSON，通过 Builder 模式转译为特定核心的配置格式
- **插件化协议**: 新增协议只需实现 `ConfigBuilder` trait，无需修改 Hub 核心逻辑

---

## 组件详解

### pp-hub — 中央管理面板

Hub 是系统的控制平面，提供三种服务：

| 服务 | 协议 | 端口 | 说明 |
|------|------|------|------|
| REST API | HTTP/1.1 | 8081 (默认) | 管理操作、查询接口 |
| gRPC | HTTP/2 | 50052 (默认) | Agent 长连接、双向流通信 |
| Web App | HTTP/1.1 | 8081 (同 API) | 静态文件服务，SPA 回退 |

**内部模块：**

```
pp-hub/
├── src/
│   ├── main.rs              # 入口：启动 HTTP + gRPC 双服务
│   ├── state.rs             # AppState：共享状态（DB + Agent 连接表）
│   ├── grpc/
│   │   └── agent_service.rs # HubAgent gRPC 服务实现
│   ├── middleware/
│   │   └── auth.rs          # JWT / Token 认证中间件（预留）
│   ├── routes/              # HTTP 路由处理器
│   │   ├── nodes.rs         # 节点 CRUD + 配置推送
│   │   ├── protocol.rs      # 协议配置 CRUD + REALITY 密钥生成
│   │   ├── client.rs        # 客户端（用户）管理
│   │   ├── bindings.rs      # 节点-配置绑定
│   │   ├── subscription.rs  # 订阅模板 + 订阅管理 + 公开订阅端点
│   │   ├── traffic.rs       # 流量查询
│   │   ├── metrics.rs       # 主机指标查询
│   │   ├── logs.rs          # 系统日志查询
│   │   └── health.rs        # 健康检查
│   └── service/             # 业务逻辑层
│       ├── node.rs          # 节点业务逻辑
│       ├── protocol.rs      # 配置生成服务
│       ├── subscription.rs  # 订阅业务逻辑
│       └── traffic.rs       # 流量统计服务
```

### pp-agent — 节点代理

Agent 部署在每个代理节点上，负责：

1. **gRPC 连接管理**: 与 Hub 建立双向流，自动重连
2. **核心进程管理**: 发现、启动、停止、重载 sing-box/mihomo
3. **指标上报**: 定时采集并上报主机 CPU、内存、网络、负载
4. **流量上报**: 从核心进程获取并上报流量统计
5. **日志上报**: 收集核心日志并批量上报

**内部模块：**

```
pp-agent/
├── src/
│   ├── main.rs              # 入口：参数解析、Token 加载、启动客户端
│   └── client.rs            # AgentStreamClient：gRPC 连接、重连、消息处理
```

### pp-web — Web 前端

基于 React 的响应式单页应用（SPA）：

- **技术栈**: React 18 + TypeScript + Vite 6 + HeroUI + Tailwind CSS v4 + react-i18next（国际化）
- **构建目标**: 静态 JavaScript / CSS 资源（由 Hub 或 CDN 托管）
- **通信方式**: 通过 Axios 调用 Hub REST API

**页面结构：**

| 路由 | 页面 | 功能 |
|------|------|------|
| `/` | Dashboard | 全局概览、统计卡片、节点状态、最近日志 |
| `/nodes` | Nodes | 节点列表、创建、删除、状态监控、父节点配置 |
| `/protocols` | Protocols | 协议配置管理、REALITY 密钥生成 |
| `/bindings` | Bindings | 节点与协议配置的绑定关系 |
| `/clients` | Clients | 客户端（用户）管理、流量配额、on-hold 状态 |
| `/groups` | Groups | 用户组管理 |
| `/subscriptions` | Subscriptions | 订阅模板与订阅链接管理 |
| `/hosts` | Hosts | 主机设置与变量管理 |
| `/metrics` | Metrics | 主机性能指标图表 |
| `/traffic` | Traffic | 实时与历史流量查询 |
| `/onlines` | Onlines | 在线用户列表 |
| `/logs` | Logs | 系统日志查看与筛选 |
| `/api-keys` | ApiKeys | Bootstrap 与管理 API Key |
| `/webhooks` | Webhooks | Webhook 事件接收配置 |

### pp-core — 核心进程管理

提供 sing-box 和 mihomo 的统一管理抽象：

```rust
/// 核心管理器接口
trait CoreManager: Send + Sync {
    fn core_type(&self) -> CoreType;
    async fn start(&self, config: &Value) -> PanelResult<()>;
    async fn stop(&self) -> PanelResult<()>;
    async fn restart(&self, config: &Value) -> PanelResult<()>;
    async fn reload(&self, config: &Value) -> PanelResult<()>;
}

/// 节点上的核心监管器
struct CoreSupervisor {
    managers: RwLock<HashMap<CoreType, Arc<dyn CoreManager>>>,
}
```

**功能：**
- 自动发现系统中的 sing-box/mihomo 二进制文件（缺失时按需从 GitHub Releases 下载安装）
- 通过子进程管理核心生命周期
- 配置文件变更监听（`notify` crate）
- 流量统计采集（通过核心提供的 API / 日志解析）

### pp-config — 配置构建器

将数据库中存储的通用协议配置转译为 sing-box 可识别的 JSON 配置（mihomo 配置同样以 JSON 形式在 Hub↔Agent 间传输，由 Agent 侧的 `MihomoProcessManager` 在落盘时序列化为 `mihomo.yaml`）。

**架构：**

```rust
trait ConfigBuilder: Send + Sync {
    fn core_type(&self) -> CoreType;
    fn build_inbound(&self, protocol: ProtocolType, settings: &Value, tls: Option<&Value>) -> PanelResult<Value>;
    fn build_full_config(&self, inbounds: &[InboundConfig]) -> PanelResult<Value>;
}
```

**实现：**
- `SingBoxConfigBuilder`: 生成 sing-box 配置
- `MihomoConfigBuilder`: 生成 mihomo 配置（listeners 结构；vless 用户为列表、hysteria2/anytls 用户为映射；TLS 支持托管证书（agent 内置 ACME 统一目录）或显式证书文件；内置 ACME 为 sing-box 专属）

**BuilderRegistry**: 运行时注册表，支持按核心类型查找对应的构建器。

### pp-subscription — 订阅生成器

将标准化的代理节点列表序列化为各种客户端支持的订阅格式。

**支持的格式：**

| 格式 | 内容类型 | 典型客户端 |
|------|----------|-----------|
| Base64 | `text/plain` | V2RayN、V2RayNG、Shadowrocket |
| JSON | `application/json` | 通用 |
| Clash | `application/x-yaml` | Clash Verge、Clash Meta |
| SingBox | `application/json` | sing-box GUI、NekoBox |
| V2RayNG | `application/json` | V2RayNG (专用格式) |

**凭证注入：**

订阅生成时会自动将客户端的 UUID/Email/Password 注入到节点配置中，实现"一链一用户"。

### pp-db — 数据库层

基于 Sea-ORM 的数据库抽象层：

- **连接管理**: 支持 PostgreSQL 和 SQLite
- **迁移系统**: 使用 `sea-orm-migration`，版本化 schema 变更
- **实体定义**: 手写实体匹配迁移 schema（生产环境建议使用 `sea-orm-cli generate entity`）

### pp-common — 共享基础设施

所有 crate 共享的基础模块：

- `models.rs`: DTO（NodeDto、ProtocolConfigDto、ClientDto）
- `protocol.rs`: 枚举（ProtocolType、CoreType、NodeStatus、UserStatus）
- `crypto.rs`: 加密工具（Token 生成、UUID、X25519 密钥对）
- `error.rs`: 全局错误类型 `PanelError` / `PanelResult<T>`

### pp-proto — gRPC 协议

由 `tonic-build` 从 `proto/hub_agent.proto` 自动生成的 Rust 代码。

---

## 数据流

### 3.1 节点注册流程

```
Agent 启动
    │
    ▼
生成 agent_id (UUID) + 加载 Token
    │
    ▼
gRPC Stream → Hub
    │
    ▼
RegisterRequest { agent_id, token, hostname, capabilities }
    │
    ▼
Hub 验证 Token → 创建/更新 Node 记录
    │
    ▼
RegisterResponse { success, heartbeat_interval }
    │
    ▼
Agent 进入心跳循环
```

### 3.2 配置推送流程

```
管理员在 Web / API 操作
    │
    ▼
POST /api/v1/nodes/{id}/push { core_type, restart }
    │
    ▼
Hub 查询该节点的所有 active Bindings
    │
    ▼
pp-config BuilderRegistry.build_full_config(inbounds)
    │
    ▼
序列化为 JSON → config_version = SHA-256(config) 前 16 位
    │
    ▼
gRPC Stream → Agent（Hub 侧对调度推送先比对 Agent 注册时上报的版本，一致则跳过）
    │
    ▼
Agent 比对本地快照版本（非 restart 推送且版本一致则跳过应用）
    │
    ▼
Agent → CoreManager.reload() / restart()
    │
    ▼
sing-box/mihomo 加载新配置
```

### 3.3 订阅服务流程

```
用户访问 /sub/{token}?format=clash
    │
    ▼
Hub 查找 Subscription 记录
    │
    ▼
获取 Client + Template + active Bindings
    │
    ▼
build_proxy_nodes() — 为每个节点注入客户端凭证
    │
    ▼
generate_subscription(Clash, nodes, base_config)
    │
    ▼
返回 YAML / JSON / Base64 内容
```

### 3.4 流量上报流程

```
sing-box/mihomo 运行中
    │
    ▼
pp-core 采集流量统计（API / 日志解析）
    │
    ▼
Agent 定期打包为 TrafficReport
    │
    ▼
gRPC Stream → Hub
    │
    ▼
Hub 将流量数据写入 traffic_records（按小时聚合）
```

### 3.5 主机指标上报流程

```
Agent 指标定时器 (默认 60s)
    │
    ▼
sysinfo 采集 CPU、内存、负载
    │
    ▼
打包为 HostMetrics
    │
    ▼
gRPC Stream → Hub
    │
    ▼
Hub 写入 host_metrics 表
```

---

## 客户端架构

桌面客户端（**Client**）运行在用户设备上，是 ProxyPanel 的用户侧组件：经由 Hub 的公开订阅端点拉取节点配置，在本地驱动 sing-box / mihomo 核心，并叠加 MITM 与脚本引擎实现 HTTPS 解密抓包、请求响应重写、QX/Surge/Loon 脚本兼容与本地定时任务。

客户端由四个 crate 组成：

| Crate | 职责 | 说明 |
|-------|------|------|
| `pp-script` | 脚本引擎层 | QuickJS 运行时 + QX/Surge/Loon 三方言 API 适配 + cron 调度 |
| `pp-mitm` | HTTPS MITM 引擎 | CA 管理、hudsucker 封装、重写 / 脚本钩子 / 抓包、上游代理 |
| `pp-client` | 客户端核心库 | 订阅同步、核心配置合成、系统代理、生命周期编排 |
| `pp-client-ui` | 桌面应用 | Tauri 2.11 + React 19 前端，5 页界面 |

### pp-script — 脚本引擎层

基于 QuickJS（`rquickjs` 0.12）的客户端代理脚本引擎：

- **引擎无关抽象**: [`ScriptEngine`] trait 定义引擎边界，为未来第二个后端（Apple JavaScriptCore，feature-gated `engine-jsc`）预留；当前后端实现见 `engine_quickjs.rs`
- **三方言 API 适配**（`dialect.rs`）: 同一 host 能力按方言注入不同全局名——QX（`$task` / `$prefs` / `$notify`）、Surge（`$httpClient` / `$persistentStore` / `$notification`）、Loon（以上全部 + `$loon` 标记，超集），统一包含 `$done` 与 `setTimeout` 注册表实现
- **`ScriptWorker` actor**（`worker.rs`）: rquickjs 的 `AsyncRuntime` 内部含非 `Send` 结构，统一收敛为「专有 OS 线程 + 专用 `current_thread` tokio runtime」，任务经 mpsc 通道串行化执行，对外暴露 `Send` future
- **`ScriptScheduler`**（`scheduler.rs`）: cron / event 脚本调度（QX/Surge/Loon 的 task/cron 签到类脚本），支持注册 / 移除、手动触发、到期批量执行；cron 表达式为 **6 段含秒**，**5 段自动补 0**
- **`FilePersistentStore`**（`host.rs`）: 每个 scope 一个 JSON 文件，文件名 = scope **SHA-256 前 16 位 hex** + `.json`，write / erase 同步**原子落盘**（tempfile + rename），重启不丢

### pp-mitm — HTTPS MITM 引擎

本地 HTTPS 中间人代理，基于 hudsucker 0.25：

- **CA 管理**: [`CaStore`] trait 抽象 CA 材料，[`FileCaStore`] 为基于本地目录的默认实现，首次调用用 **rcgen** 生成自签证书（`ca.crt` / `ca.key`，文件权限 **0600**）
- **hudsucker 封装**: 白名单双钩子 passthrough——`should_intercept_connect`（CONNECT 按主机名白名单判定是否拦截）/ `should_intercept_tls`，白名单外流量整体透传
- **`RewriteEngine`**（`rewrite.rs`）: URL / Header / Body / Reject / Mock 五类重写动作
- **`ScriptHookEngine`**（`script_hook.rs`）: 将 http-request / http-response 类型脚本按 URL 规则挂载到流量路径，成功时回写输出，脚本**超时或抛异常时 no-op（透传原值）**并记录警告
- **流量抓包**: [`TrafficRecorder`] trait + [`MemoryRecorder`] 环形缓冲实现（`recorder.rs`）
- **上游代理链**（`upstream.rs`）: [`UpstreamProxy`]（Direct / HTTP CONNECT 父代理 / SOCKS5 父代理），经实现 [`tower::Service<Uri>`] 的自定义 connector 接入 hudsucker 的 `with_http_connector` 扩展点

### pp-client — 桌面客户端核心库

承载客户端全部业务逻辑，供 pp-client-ui 调用：

- **配置**: `ClientConfig`（`client.json`）定义客户端配置，含 `mixed_port`（默认 17890）与 MITM 配置
- **订阅同步**（`subscription.rs`）: 拉取 `?format=singbox` / `?format=clash` 订阅，解析 `subscription-userinfo` 响应头（upload / download / total / expire）
- **核心配置合成**（`core_config.rs`）: 将订阅配置合成为本地核心启动配置，构建 **MITM 链路**——sing-box 为双 mixed inbound（主入口 `main-in` + 回流入口 `mitm-return`），route 规则前插 `inbound = [main-in]` 白名单规则；mihomo 用显式 listeners 声明替代顶层 mixed-port，rules 前插 `AND((IN-NAME,main-in),(DOMAIN-SUFFIX,<suffix>)),pp-mitm` / `AND((IN-NAME,main-in),(DOMAIN,<exact>)),pp-mitm` 规则
- **MITM 编排**（`mitm.rs`）: MITM 上游指向本机核心回流 mixed 入站端口（默认 `mixed_port + 1`），解密后的流量回流核心继续正常路由
- **核心进程**（`runner.rs`）: `CoreRunner` 封装 pp-core 的 `CoreManagerFactory`，管理 sing-box / mihomo 子进程
- **系统代理**（`sysproxy.rs`）: `SystemProxy` trait + 平台实现——macOS `networksetup`、Windows `reg add`（Internet Settings）、Linux `gsettings`（GNOME），命令构造为纯函数便于单测断言，可注入 `MockSystemProxy`
- **远程订阅**（`remote.rs`）: `RemoteManager` 定时拉取远程脚本 / 重写片段，写本地缓存（`data_dir/remote_cache/<name>.json`），运行期 `load_cached` 合并
- **导入**（`import.rs`）: 把 QX / Surge / Loon 的 rewrite / script / task / mitm 片段经 `parse_import` 解析为 pp-mitm 规则与 pp-script 任务，未知行跳过并记 warning
- **生命周期编排**（`state.rs`）: `ClientState` 编排启动链（订阅 → MITM → 合成配置 → 核心 → 系统代理），任一步失败**逆序回滚**，notifier / sysproxy 可注入

### pp-client-ui — 桌面应用

- **技术栈**: Tauri 2.11（独立 cargo 项目，**退出根 workspace**）+ React 19 / Vite 8 / TypeScript 7 / Tailwind CSS 4 / HeroUI 3.2，Bun 作为包管理器
- **页面**: 仪表盘（`/`）、节点（`/nodes`）、MITM（`/mitm`）、脚本（`/scripts`）、设置（`/settings`）共 5 页
- **通知**: `tauri-plugin-notification` 桌面通知

### 客户端流量链路

桌面客户端的流量链路（核心主入口 → MITM → 核心回流）是理解 MITM 挂载方式的关键：

```
App → 系统代理 → 核心主 mixed inbound (mixed_port)
   ├─ MITM 白名单域名 → route 规则（inbound=main-in）→ http outbound → pp-mitm（CA 解密+脚本钩子+rewrite+抓包）
   │     → UpstreamProxy::Http → 核心 mitm-return inbound (mixed_port+1) → 正常路由 → 远端节点
   └─ 其余流量（含 wss）→ 正常路由 → 远端节点
```

1. App 的请求经系统代理指向核心主 mixed 入站（`main-in`，监听 `mixed_port`）
2. 命中 MITM 白名单的域名：由 route 白名单规则（sing-box `inbound = [main-in]` / mihomo `AND((IN-NAME,main-in),…)`）匹配，经 http outbound 转发到 `pp-mitm`
3. pp-mitm 完成 CA 解密、脚本钩子、重写与抓包后，经 `UpstreamProxy::Http` 转发到核心回流入站（`mitm-return`，监听 `mixed_port + 1`），回到核心后走正常路由链到远端节点
4. 其余流量（含 wss）不经过 MITM，直接正常路由到远端节点

### 关键设计决策

- **ScriptWorker !Send 执行模型**: rquickjs 的 `AsyncRuntime` 非 `Send`，若直接跨线程使用会反复创建运行时；收敛为「单一专有线程 + mpsc 串行化」后对外暴露 `Send` future，脚本超时 / 异常由引擎隔离
- **MITM 挂核心后 + 防回环**: MITM 不作为系统代理入口，而是核心链路中的一环（http outbound）；主 / 回流双 mixed inbound + 白名单路由规则保证只解密白名单域名，且 MITM 上游指向核心自己的回流入站而非系统代理，避免解密流量再次进入主入口形成无限循环
- **上游代理链**: MITM 解密后的流量经 `UpstreamProxy`（HTTP CONNECT / SOCKS5）返回核心回流，使解密流量与正常流量共享同一路由与节点选择，保证出口一致性
- **三方言 API 兼容策略**: 按 `ScriptDialect` 注入不同的全局名集合（QX / Surge / Loon），Loon 为超集；同一份 host 能力共享，仅全局名不同，脚本生态无需改写即可迁移
- **QuickJS vs JSC 双引擎规划**: `ScriptEngine` trait 将引擎与方言 API 解耦，QuickJS（rquickjs 0.12）为当前默认后端，Apple JSC 通过 `engine-jsc` feature 接入，构造签名保持一致

---

## 数据库设计

### E-R 关系图

```
┌─────────────┐       ┌──────────────────┐       ┌─────────────────┐
│    Users    │       │      Nodes       │       │ ProtocolConfigs │
├─────────────┤       ├──────────────────┤       ├─────────────────┤
│ id (PK)     │       │ id (PK)          │       │ id (PK)         │
│ username    │       │ name             │       │ name            │
│ password_hash│      │ hostname         │       │ protocol_type   │
│ role        │       │ address          │       │ core_type       │
│ status      │       │ token_hash       │       │ listen_port     │
└─────────────┘       │ cores_available  │       │ listen_address  │
       │              │ labels           │       │ settings (JSON) │
       │              │ status           │       │ tls_settings    │
       │              └──────────────────┘       └─────────────────┘
       │                      │                          │
       │                      │     ┌──────────────┐     │
       │                      └────►│ NodeBindings │◄────┘
       │                            ├──────────────┤
       │                            │ id (PK)      │
       │                            │ node_id (FK) │
       │                            │ protocol_config_id (FK)
       │                            │ override_settings
       │                            │ is_active    │
       │                            └──────────────┘
       │
       ▼
┌─────────────┐       ┌──────────────────┐       ┌─────────────────┐
│   Clients   │       │  Subscriptions   │       │SubscriptionTemplates
├─────────────┤       ├──────────────────┤       ├─────────────────┤
│ id (PK)     │◄──────│ client_id (FK)   │       │ id (PK)         │
│ user_id (FK)│       │ template_id (FK) │──────►│ name            │
│ name        │       │ token (unique)   │       │ format          │
│ email       │       │ url_path         │       │ base_config     │
│ traffic_limit│      │ expire_at        │       │ filter_rules    │
│ traffic_used │      │ is_active        │       │ custom_headers  │
│ status      │       └──────────────────┘       └─────────────────┘
└─────────────┘
       │
       ▼
┌─────────────────┐   ┌─────────────────┐   ┌─────────────────┐
│ TrafficRecords  │   │   HostMetrics   │   │   SystemLogs    │
├─────────────────┤   ├─────────────────┤   ├─────────────────┤
│ id (PK)         │   │ id (PK)         │   │ id (PK)         │
│ node_id (FK)    │   │ node_id (FK)    │   │ level           │
│ protocol_config_id│  │ timestamp       │   │ source          │
│ client_id (FK)  │   │ cpu_percent     │   │ message         │
│ hour_bucket     │   │ mem_used        │   │ metadata (JSON) │
│ upload_bytes    │   │ mem_total       │   │ created_at      │
│ download_bytes  │   │ disk_used       │   └─────────────────┘
└─────────────────┘   │ net_rx          │
                      │ load_avg*       │
                      └─────────────────┘
```

### 表说明

| 表名 | 说明 | 关键索引 |
|------|------|----------|
| `users` | 系统用户（管理员） | `username` (unique) |
| `nodes` | 代理节点 | `status`, `last_seen_at` |
| `protocol_configs` | 协议入站配置 | `protocol_type`, `core_type` |
| `node_bindings` | 节点与配置的绑定关系 | `node_id`, `protocol_config_id` |
| `relay_rules` | 服务端中继路由：入口节点按规则集/域名分流到出口绑定 | `node_id`, `enabled` |
| `node_pending_updates` | 节点待更新脏标记：协议/绑定/中继/版本变更写入，手动推送后消除 | `node_id`, `core_type`, `update_type` |
| `clients` | 代理客户端（最终用户） | `user_id`, `status` |
| `subscription_templates` | 订阅输出格式模板 | — |
| `subscriptions` | 客户端订阅记录 | `token` (unique) |
| `traffic_records` | 流量统计（按小时聚合） | `hour_bucket`, `node_id` |
| `host_metrics` | 主机性能指标 | `node_id`, `timestamp` |
| `system_logs` | 系统/Agent 日志 | `source`, `created_at` |

---

## 通信协议

### gRPC 双向流 (`proto/hub_agent.proto`)

**服务定义：**

```protobuf
service HubAgent {
  rpc Stream(stream AgentMessage) returns (stream HubMessage);
}
```

**Agent → Hub 消息类型：**

| 消息 | 触发条件 | 频率 |
|------|----------|------|
| `RegisterRequest` | 连接建立 | 每次连接 |
| `Heartbeat` | 定时 | 默认 30s |
| `TrafficReport` | 定时 | 默认 60s |
| `HostMetrics` | 定时 | 默认 60s |
| `LogBatch` | 日志积累或定时 | 批量 |
| `CoreStatusReport` | 状态变化 | 事件驱动 |

**Hub → Agent 消息类型：**

| 消息 | 触发条件 | 效果 |
|------|----------|------|
| `RegisterResponse` | 收到注册请求 | 确认注册，分配 ID |
| `ConfigPush` | 管理员推送配置 | 写入并重启/重载核心 |
| `ConfigReload` | 热重载请求 | 仅重载不重启 |
| `CoreCommand` | 核心控制命令 | 启动/停止/重启核心 |
| `AgentShutdown` | 远程关机指令 | 延迟后退出进程 |

### HTTP REST API

详见 [api_reference.md](api_reference.md)。

---

## 安全模型

### 认证机制

| 层级 | 机制 | 状态 |
|------|------|------|
| Agent → Hub | Token 预共享密钥 | ✅ 已实现 |
| Web → Hub | JWT Bearer Token | 🚧 预留（middleware/auth.rs） |
| 订阅端点 | URL Token (随机字符串) | ✅ 已实现 |

### Token 安全

- Agent Token: 32 bytes 加密安全随机数，Base64 编码（43 字符）
- 订阅 Token: 同上，独立生成
- REALITY 私钥: X25519 静态密钥，Base64 编码

### 传输安全

- 生产环境应在 Hub 前部署 TLS 终止（Nginx / Caddy / Cloudflare）
- gRPC 支持 TLS（Tonic 已配置 `tls-ring` feature）
- Agent 与 Hub 的 gRPC 连接建议通过内网或 VPN

---

## 扩展点

### 添加新的代理协议

1. 在 `ProtocolType` 添加变体
2. 在 `pp-config` 的 sing-box/mihomo builder 中实现 `build_inbound`
3. 在 `pp-subscription` 各格式中实现节点序列化
4. 在 `validate_protocol` 中注册核心兼容性

### 添加新的订阅格式

1. 实现 `fn generate(nodes: &[ProxyNode]) -> PanelResult<String>`
2. 在 `SubscriptionFormat` 添加变体
3. 在 `generate_subscription` 中添加分发分支

### 添加新的核心类型

1. 在 `CoreType` 添加变体
2. 实现 `CoreManager` trait
3. 在 `CoreSupervisor::discover` 中添加发现逻辑
4. 在 `BuilderRegistry` 中注册对应的 `ConfigBuilder`

---

## 性能考量

- **数据库连接池**: Sea-ORM 内部管理，默认大小适应 Tokio 线程池
- **Agent 连接数**: 内存 HashMap 存储，单 Hub 可支撑数千节点
- **流量聚合**: 按小时桶聚合，避免单条记录过多
- **日志批量**: Agent 端批量上报，减少 gRPC 消息数量
- **配置缓存**: Hub 可缓存生成的配置 JSON，减少重复构建
