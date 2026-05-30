# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build, Test & Development Commands

```bash
# Compile everything
cargo build --workspace

# Run tests (uses SQLite in-memory for DB tests)
cargo test --workspace
cargo test -p pp-common           # single crate
cargo test --workspace -- --nocapture

# Lint & format
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all

# Run Hub (requires PostgreSQL or SQLite)
cargo run --bin proxy-panel-hub
RUST_LOG=proxy_panel_hub=debug cargo run --bin proxy-panel-hub

# Run Agent
cargo run --bin proxy-panel-agent -- --hub-url "http://localhost:50052"

# Web frontend (Dioxus)
cd crates/pp-web
dx serve                          # dev server with HMR
dx build --release              # production WASM build

# Database
cargo run --bin proxy-panel -- init-db --database-url "postgres://proxypanel:proxypanel@localhost/proxypanel"
# or SQLite for quick testing:
export PROXYPANEL_DATABASE_URL="sqlite://./dev.db?mode=rwc"
cargo run --bin proxy-panel -- init-db --database-url "$PROXYPANEL_DATABASE_URL"

# Regenerate Sea-ORM entities after schema changes
cd crates/pp-db
sea-orm-cli generate entity \
  --database-url "postgres://proxypanel:proxypanel@localhost/proxypanel" \
  -o src/entities

# gRPC debugging
grpcurl -plaintext localhost:50052 list
grpcurl -plaintext -proto proto/hub_agent.proto localhost:50052 list proxypanel.HubAgent
```

## High-Level Architecture

### 1. Protocol Configuration Pipeline (Multi-Kernel Translation)

The central design problem: the panel stores protocol configs in a **kernel-neutral** format, then translates them to xray-core or sing-box JSON at push/subscription time.

**Key files to read together:**
- `crates/pp-common/src/protocol.rs` — `ProtocolType` enum defines supported protocols
- `crates/pp-config/src/builder.rs` — `ConfigBuilder` trait: `build_inbound(protocol, settings, tls) -> JSON`
- `crates/pp-config/src/xray.rs` — `XrayConfigBuilder` implementation
- `crates/pp-config/src/singbox.rs` — `SingBoxConfigBuilder` implementation
- `crates/pp-hub/src/service/protocol.rs` — `generate_node_config()` orchestrates: query bindings → parse protocol → merge overrides → call builder
- `crates/pp-hub/src/routes/protocol.rs` — `validate_protocol()` enforces which cores support which protocols

**Neutral fields convention:** Frontend stores fields like `reality_dest`, `reality_private_key`, `xhttp_path`, `xhttp_mode`. The builders convert these to kernel-specific names (`privateKey` for xray, `private_key` for sing-box). Both old and new field names are accepted for backward compatibility.

**User credential flow:**
- Subscriptions: `crates/pp-hub/src/routes/subscription.rs` `inject_client_credentials()` injects a `clients` array into `settings`
- Builders then convert `clients` to the target kernel's format:
  - xray VLESS: `settings.clients` (id, email, flow)
  - sing-box VLESS/Hysteria2/AnyTLS/TUIC: `users` (uuid/name, password)

### 2. Hub-Agent Communication

**gRPC bidirectional streaming** defined in `proto/hub_agent.proto`. A single `rpc Stream` carries both directions.

**Agent → Hub messages:** RegisterRequest, Heartbeat, TrafficReport, HostMetrics, LogBatch, CoreStatusReport  
**Hub → Agent messages:** RegisterResponse, ConfigPush, ConfigReload, CoreCommand, AgentShutdown

**Connection management:**
- `crates/pp-hub/src/state.rs` — `AppState` holds `Arc<RwLock<HashMap<Uuid, AgentConnection>>>` for live agent connections
- `crates/pp-hub/src/grpc/agent_service.rs` — `HubAgentService` implements the gRPC stream handler
- `crates/pp-agent/src/client.rs` — `AgentStreamClient` manages connection, auto-reconnect, and message dispatch

**Config push path:**
1. Admin calls `POST /api/v1/nodes/{id}/push` (`routes/nodes.rs`)
2. `service/protocol.rs::generate_node_config()` queries active `node_bindings` + `protocol_configs`
3. Merges `override_settings` from binding into config `settings`
4. Looks up `BuilderRegistry` by target core (xray/sing-box)
5. Serializes full config JSON and sends via `state.send_to_agent()` → gRPC `ConfigPush`
6. Agent receives it, writes to temp file, calls `CoreManager.reload()` or `restart()`

### 3. Subscription Generation

**Files:**
- `crates/pp-hub/src/routes/subscription.rs` — `serve_subscription()` endpoint: validates token → builds proxy nodes → injects credentials → calls generator
- `crates/pp-subscription/src/generator.rs` — `SubscriptionFormat` enum + dispatch
- `crates/pp-subscription/src/formats/base64.rs` — Traditional URL links (vless://, vmess://, trojan://, ss://)
- `crates/pp-subscription/src/formats/singbox.rs` — sing-box JSON outbounds
- `crates/pp-subscription/src/formats/clash.rs` — Clash Meta YAML

Subscription templates (`subscription_templates` table) store `base_config` and `format`. The endpoint `/sub/{token}?format=xxx` serves the generated config.

### 4. Database & Migrations

- **Library crate:** `crates/pp-db`
- **Migrations:** `crates/pp-db/src/migration/`
- **Entities:** `crates/pp-db/src/entities/` (hand-maintained; regenerate with `sea-orm-cli` after schema changes)
- **Connection:** `pp_db::init_db(url)` supports both PostgreSQL and SQLite
- **Migration runner:** `pp_db::run_migrations(&db)`

Key tables: `nodes`, `protocol_configs` (settings + tls_settings as JSON), `node_bindings` (many-to-many with override_settings), `clients`, `subscriptions`, `traffic_records`, `host_metrics`.

## Commit Conventions

This project uses **Conventional Commits** combined with **atomic commits** (one logical change per commit, must compile and pass tests independently).

Format: `<type>(<scope>): <subject>`

**Types:** `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `chore`, `ci`, `security`

**Scopes:** `hub`, `agent`, `web`, `cli`, `db`, `config`, `core`, `sub`, `proto`, `common`, `docs`

Examples:
```
feat(hub): 添加节点批量导入 API
fix(agent): 修复断连后指数退避计算溢出的问题
refactor(db): 提取节点查询为 NodeService 方法
docs: 更新部署指南中的 TLS 配置示例
```

## Extending the System

### Adding a New Proxy Protocol

1. Add variant to `ProtocolType` in `crates/pp-common/src/protocol.rs`
2. Implement `build_inbound` in both `crates/pp-config/src/xray.rs` and `singbox.rs`
3. Add validation in `crates/pp-hub/src/routes/protocol.rs` `validate_protocol()`
4. Add parsing in `crates/pp-hub/src/service/protocol.rs` `parse_protocol_type()`
5. Add credential injection in `crates/pp-hub/src/routes/subscription.rs` `inject_client_credentials()`
6. Add subscription serialization if applicable (base64.rs, clash.rs, singbox.rs)
7. Add frontend form fields in `crates/pp-web/src/pages/protocols.rs`

### Adding a New Subscription Format

1. Add variant to `SubscriptionFormat` in `crates/pp-subscription/src/generator.rs`
2. Implement `generate(nodes, base_config)` in `crates/pp-subscription/src/formats/`
3. Wire up in `generate_subscription()` dispatch
4. Add content-type mapping in `crates/pp-hub/src/routes/subscription.rs` `serve_subscription()`

## Important Implementation Notes

- **Rust edition 2024**, minimum version 1.86. The workspace `Cargo.toml` defines shared dependencies.
- **No `unwrap()` in production code** (initialization and test setup are exceptions).
- **Error type:** `pp_common::PanelError` / `PanelResult<T>` is used across all crates.
- **Frontend API base URL:** `crates/pp-web/src/api.rs` currently hardcodes `http://localhost:8081` for development.
- **Hub serves static files:** `proxy-panel-hub --static-dir crates/pp-web/dist` serves the Dioxus SPA with fallback to `index.html`.
- **TLS for REALITY:** sing-box REALITY config does NOT use `certificate_path`/`key_path`; it uses `tls.reality.handshake` to copy the target site's TLS fingerprint.
- **XHTTP is xray-only:** sing-box does not support the `xhttp` V2Ray transport type.
