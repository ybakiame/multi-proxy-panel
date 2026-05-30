# ProxyPanel 开发指南

本文档面向项目开发者，描述开发环境搭建、常用任务、调试方法和代码贡献流程。

---

## 目录

1. [环境准备](#环境准备)
2. [项目初始化](#项目初始化)
3. [开发工作流](#开发工作流)
4. [数据库开发](#数据库开发)
5. [前端开发](#前端开发)
6. [测试](#测试)
7. [调试技巧](#调试技巧)
8. [代码审查清单](#代码审查清单)

---

## 环境准备

### 必需工具

| 工具 | 版本 | 用途 |
|------|------|------|
| Rust | 1.86+ | 后端与核心开发 |
| PostgreSQL | 15+ | 开发数据库 |
| Docker & Compose | 最新 | 基础设施快速启动 |
| dioxus-cli (`dx`) | 0.7+ | Web 前端构建 |
| sea-orm-cli | 1.1+ | 数据库实体生成 |
| grpcurl | 最新 | gRPC 接口调试 |

### 安装 Rust 工具链

```bash
# 项目已包含 rust-toolchain.toml，自动安装正确版本
cd proxy-panel
rustc --version  # 应显示 1.86+

# 安装额外组件
cargo install dioxus-cli
cargo install sea-orm-cli
```

### 安装系统依赖

**Ubuntu/Debian:**

```bash
sudo apt-get update
sudo apt-get install -y libssl-dev pkg-config protobuf-compiler
```

**macOS:**

```bash
brew install protobuf
```

---

## 项目初始化

### 1. 克隆仓库

```bash
git clone https://github.com/your-org/proxy-panel.git
cd proxy-panel
```

### 2. 启动基础设施

```bash
# 启动 PostgreSQL
docker compose up -d postgres

# 验证数据库就绪
docker compose exec postgres pg_isready -U proxypanel
```

### 3. 初始化数据库

```bash
# 运行迁移
cargo run --bin proxy-panel -- init-db \
  --database-url "postgres://proxypanel:proxypanel@localhost/proxypanel"
```

### 4. 验证编译

```bash
# 编译全部 crate
cargo build --workspace

# 运行测试
cargo test --workspace
```

---

## 开发工作流

### 日常开发循环

```bash
# 1. 拉取最新代码
git pull origin main

# 2. 创建功能分支
git checkout -b feature/your-feature

# 3. 开发...

# 4. 格式化代码
cargo fmt --all

# 5. 静态检查
cargo clippy --workspace --all-targets -- -D warnings

# 6. 运行测试
cargo test --workspace

# 7. 提交代码
git add .
git commit -m "feat: your feature description"
```

### 启动开发环境

**终端 1 — 启动 Hub:**

```bash
RUST_LOG=proxy_panel_hub=debug,tower_http=debug \
  cargo run --bin proxy-panel-hub
```

**终端 2 — 启动 Agent（可选）:**

```bash
RUST_LOG=proxy_panel_agent=debug \
  cargo run --bin proxy-panel-agent \
  -- --hub-url "http://localhost:50052"
```

**终端 3 — 启动前端开发服务器:**

```bash
cd crates/pp-web
dx serve
```

访问 `http://localhost:8080`（前端开发服务器，带热重载）

---

## 数据库开发

### 添加新迁移

```bash
cd crates/pp-db

# 创建新迁移（使用 sea-orm-cli）
sea-orm-cli migrate generate create_new_table

# 编辑生成的迁移文件
code src/migration/m2025xxxx_xxxxxx_create_new_table.rs
```

### 迁移文件模板

```rust
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MyTable::Table)
                    .if_not_exists()
                    .col(pk_uuid(MyTable::Id))
                    .col(string(MyTable::Name))
                    .col(timestamp(MyTable::CreatedAt))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(MyTable::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum MyTable {
    Table, Id, Name, CreatedAt,
}
```

### 注册迁移

在 `crates/pp-db/src/migration/mod.rs` 中添加：

```rust
mod m2025xxxx_xxxxxx_create_new_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250101_000001_create_initial_tables::Migration),
            Box::new(m2025xxxx_xxxxxx_create_new_table::Migration),
        ]
    }
}
```

### 生成实体

迁移应用后，生成对应的 Sea-ORM 实体：

```bash
cd crates/pp-db
sea-orm-cli generate entity \
  --database-url "postgres://proxypanel:proxypanel@localhost/proxypanel" \
  -o src/entities
```

### 使用 SQLite 进行快速测试

```bash
# 使用 SQLite 避免启动 PostgreSQL
export PROXYPANEL_DATABASE_URL="sqlite://./dev.db?mode=rwc"
cargo run --bin proxy-panel -- init-db --database-url "$PROXYPANEL_DATABASE_URL"
```

---

## 前端开发

### 技术栈

- **框架**: Dioxus 0.7 (Rust → WASM)
- **样式**: Tailwind CSS
- **国际化**: dioxus-i18n
- **HTTP 客户端**: reqwest (WASM 兼容)

### 项目结构

```
crates/pp-web/
├── src/
│   ├── main.rs              # 入口
│   ├── app.rs               # 路由与布局
│   ├── api.rs               # HTTP API 封装
│   ├── i18n.rs              # 国际化配置
│   ├── components/          # 可复用组件
│   │   ├── data_table.rs
│   │   ├── form_input.rs
│   │   ├── modal.rs
│   │   ├── node_row.rs
│   │   └── status_badge.rs
│   └── pages/               # 页面组件
│       ├── dashboard.rs
│       ├── nodes.rs
│       ├── protocols.rs
│       ├── bindings.rs
│       ├── clients.rs
│       ├── subscriptions.rs
│       ├── metrics.rs
│       └── logs.rs
├── assets/
│   ├── tailwind.css         # Tailwind 入口
│   └── style.css            # 自定义样式
└── Cargo.toml
```

### 添加新页面

1. 在 `src/pages/` 创建新页面组件
2. 在 `src/pages/mod.rs` 导出
3. 在 `src/app.rs` 的 `Route` 枚举中添加路由
4. 在 `Layout` 的导航栏中添加链接
5. 在 `src/i18n.rs` 中添加翻译键

### API 调用示例

```rust
// crates/pp-web/src/api.rs
use serde_json::Value;

const API_BASE: &str = "/api/v1";

pub async fn get_nodes() -> reqwest::Result<Value> {
    reqwest::get(format!("{}{}/nodes", API_BASE, ""))
        .await?
        .json()
        .await
}
```

### 国际化

翻译文件位于 `src/i18n.rs`（当前为硬编码，可扩展为加载外部 JSON）：

```rust
pub fn init_i18n() {
    use dioxus_i18n::prelude::*;
    use dioxus_i18n::unic_langid::langid;

    let en_us = langid!("en-US");
    let zh_cn = langid!("zh-CN");

    I18n::new(vec![en_us, zh_cn])
        .with_default_lang(en_us)
        .with_translation(zh_cn, translate!("nav-dashboard" => "仪表盘", ...))
        .init();
}
```

---

## 测试

### 运行测试

```bash
# 全部测试
cargo test --workspace

# 指定 crate
cargo test -p pp-common
cargo test -p pp-db

# 包含被忽略的长期测试
cargo test --workspace -- --ignored

# 显示输出
cargo test --workspace -- --nocapture
```

### 编写测试

**单元测试（内联）:**

```rust
// src/lib.rs
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(1, 2), 3);
    }
}
```

**异步测试:**

```rust
#[tokio::test]
async fn test_db_operation() {
    let db = pp_db::init_db("sqlite::memory:").await.unwrap();
    // ... 测试逻辑
}
```

### 测试数据库

建议使用内存 SQLite 进行单元测试：

```rust
async fn setup_test_db() -> DatabaseConnection {
    let db = pp_db::init_db("sqlite::memory:").await.unwrap();
    pp_db::run_migrations(&db).await.unwrap();
    db
}
```

---

## 调试技巧

### 日志级别控制

```bash
# Hub 详细日志
RUST_LOG=proxy_panel_hub=debug,sea_orm=debug,tower_http=debug

# Agent 详细日志
RUST_LOG=proxy_panel_agent=debug,pp_core=debug

# 仅显示错误
RUST_LOG=error
```

### 使用 tokio-console 调试异步任务

```bash
# 启用 tokio tracing
cargo run --bin proxy-panel-hub --features tokio/tracing

# 启动 console
cargo install tokio-console
tokio-console
```

### gRPC 调试

```bash
# 列出服务
grpcurl -plaintext localhost:50052 list

# 列出方法
grpcurl -plaintext localhost:50052 list proxypanel.HubAgent

# 手动调用（需要 proto 文件）
grpcurl -plaintext -proto proto/hub_agent.proto \
  -d '{"agent_id":"...","token":"..."}' \
  localhost:50052 proxypanel.HubAgent/Stream
```

### HTTP API 调试

```bash
# 健康检查
curl http://localhost:8081/health

# 创建节点
curl -X POST http://localhost:8081/api/v1/nodes \
  -H "Content-Type: application/json" \
  -d '{"name":"test-node","hostname":"node1.example.com","address":"1.2.3.4"}'

# 获取订阅
curl http://localhost:8081/sub/your-token?format=json
```

### 数据库调试

```bash
# 进入 PostgreSQL 容器
docker compose exec postgres psql -U proxypanel -d proxypanel

# 常用查询
\dt                    # 列出表
SELECT * FROM nodes;   # 查看节点
SELECT * FROM clients; # 查看客户端
```

---

## 代码审查清单

提交 PR 前，请确认以下事项：

### 功能性

- [ ] 新功能有对应的测试覆盖
- [ ] 手动测试通过（至少运行一次完整流程）
- [ ] 错误路径已处理（如数据库连接失败、网络超时）

### 代码质量

- [ ] `cargo fmt --all` 已执行
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 无警告
- [ ] `cargo test --workspace` 全部通过
- [ ] 无裸 `unwrap()` / `expect()`（初始化代码除外）
- [ ] 新增公开的 API 有文档注释 (`///`)

### 安全性

- [ ] 用户输入已验证和清理
- [ ] 无硬编码密钥或密码
- [ ] 数据库查询无 SQL 注入风险（使用 Sea-ORM 参数绑定）

### 文档

- [ ] README.md 已更新（如添加新功能或变更使用方式）
- [ ] AGENTS.md 已更新（如变更架构或规范）
- [ ] `docs/` 下相关文档已更新
- [ ] 变更日志已记录（如项目使用 CHANGELOG）

---

## 常见问题

### Q: 编译失败，提示 protobuf 相关错误？

确保已安装 `protobuf-compiler`：

```bash
# Ubuntu/Debian
sudo apt-get install protobuf-compiler

# macOS
brew install protobuf
```

### Q: Web 前端编译后无法访问？

确保 Hub 的 `--static-dir` 指向正确路径：

```bash
cargo run --bin proxy-panel-hub -- --static-dir crates/pp-web/dist
```

### Q: Agent 无法连接 Hub？

检查：
1. Hub 的 gRPC 端口是否开放 (`50052`)
2. 防火墙是否允许连接
3. Agent 的 `--hub-url` 是否正确（需包含 `http://` 或 `https://`）

### Q: 数据库迁移失败？

```bash
# 重置开发数据库
docker compose down -v
docker compose up -d postgres
cargo run --bin proxy-panel -- init-db --database-url "postgres://proxypanel:proxypanel@localhost/proxypanel"
```
