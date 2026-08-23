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
| Node.js & npm | 20+ | 构建 Web 前端 |
| sea-orm-cli | 1.1+ | 数据库实体生成 |
| grpcurl | 最新 | gRPC 接口调试 |

### 安装 Rust 工具链

```bash
# 项目已包含 rust-toolchain.toml，自动安装正确版本
cd proxy-panel
rustc --version  # 应显示 1.86+

# 安装额外组件
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
git clone https://github.com/ybakiame/multi-proxy-panel.git
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

开发环境默认在 `config/hub.toml` 中开启 `auto_register_agents = true`，Agent 首次连接时会自动在 Hub 中注册为新节点。

```bash
RUST_LOG=proxy_panel_agent=debug \
  cargo run --bin proxy-panel-agent \
  -- --hub-url "http://localhost:50052" \
     --name "dev-agent-node" \
     --data-dir /tmp/proxypanel-agent
```

> 生产环境请关闭 `auto_register_agents`（或通过环境变量 `PROXYPANEL_AUTO_REGISTER_AGENTS=false` 覆盖），先在 **节点管理** 中创建节点并获取 token，再通过 `--token <token>` 启动 Agent。

**终端 3 — 启动前端开发服务器:**

```bash
cd crates/pp-web
bun install
bun run dev
```

访问 `http://localhost:5173`（Vite 开发服务器，带热重载）。
首次打开页面会要求输入 **API Key**，可从 Hub 启动日志中找到 Bootstrap API Key：

```bash
grep "BOOTSTRAP API KEY" scripts/.dev-logs/hub.log
```

> 注意：Bootstrap Key 是 base64 编码，**末尾的 `=` 是 key 的一部分**，日志行后面的 `.` 只是标点符号，不要复制进去。

> 若使用 `./scripts/dev.sh start`，脚本会自动设置 `PROXYPANEL_API_URL` 并打印该 Key。

**注意：5173 与 8081 的区别**

- `http://localhost:5173` 是 Vite 开发服务器，推荐使用。
- `http://localhost:8081` 是 Hub 自身的 HTTP 端口，会回退提供前端静态文件。由于静态文件没有鉴权，若此前在同一 Origin 登录过（`localStorage` 中已有 `pp_api_key`），直接访问 8081 会进入 Dashboard；所有 `/api/v1/*` 接口仍然需要 API Key。

---

## 持续集成

项目使用 GitHub Actions 进行持续集成，定义于 `.github/workflows/`：

### CI (`.github/workflows/ci.yml`)

在每次 push 到 `main`/`master` 或提交 Pull Request 时触发，包含两个并行 Job：

| Job | 说明 |
|-----|------|
| `rust` | 检查代码格式化 (`cargo fmt --check`)、运行 Clippy (`cargo clippy --workspace --all-targets -- -D warnings`)、执行测试 (`cargo test --workspace`) |
| `web` | 在 `crates/pp-web` 目录执行 `bun run verify`（构建 + oxc Linter + 格式检查） |

### Release (`.github/workflows/release.yml`)

在推送 `v*` 标签或手动触发时执行，包含四个阶段：

1. **`web`** — 构建前端产物
2. **`build`** — 在 x86_64 与 aarch64  runner 上交叉编译 Release 二进制，打包为 `proxy-panel-{hub,agent}-linux-{arch}.tar.gz`
3. **`release`** — 汇总 tar.gz、生成 `SHA256SUMS`、创建 GitHub Release（自动识别 prerelease）
4. **`docker`** — 构建并推送 GHCR 镜像 `ghcr.io/ybakiame/proxy-panel-hub` 与 `ghcr.io/ybakiame/proxy-panel-agent`

提交 PR 前请确保本地已通过 `cargo clippy --workspace --all-targets -- -D warnings` 和 `cargo test --workspace`（后端）以及 `bun run verify`（前端）。

---

## CLI 命令一览

`proxy-panel` CLI 除开发常用的 `init-db`、`create-user`、`diagnose` 等命令外，还提供生产环境组件生命周期管理能力：

| 子命令 | 说明 | 示例 |
|--------|------|------|
| `init-db` | 初始化数据库并运行迁移 | `cargo run --bin proxy-panel -- init-db --database-url "..."` |
| `create-user` | 创建管理员用户 | `cargo run --bin proxy-panel -- create-user --database-url "..." --username admin --password "..."` |
| `create-api-key` | 创建 API Key | `cargo run --bin proxy-panel -- create-api-key --database-url "..."` |
| `provision-node` | 在数据库中注册节点并生成 token | `cargo run --bin proxy-panel -- provision-node --database-url "..." --name "node-01"` |
| `gen-token` | 生成安全随机 token | `cargo run --bin proxy-panel -- gen-token` |
| `agent-token` | 生成 Agent 注册 token（带节点名标注） | `cargo run --bin proxy-panel -- agent-token --node-name "node-01"` |
| `diagnose` | 数据库连接诊断 | `cargo run --bin proxy-panel -- diagnose --database-url "..."` |
| `install hub` | 安装 Hub 组件（下载、配置、写 unit） | `sudo proxy-panel install hub` |
| `install agent` | 安装 Agent 组件（下载、配置、启动） | `sudo proxy-panel install agent --hub-url ... --token ...` |
| `upgrade <component>` | 升级组件（hub / agent / cli），失败自动回滚 | `sudo proxy-panel upgrade agent` |
| `rollback <component>` | 回滚到备份版本 | `sudo proxy-panel rollback hub` |
| `uninstall <component>` | 卸载组件 | `sudo proxy-panel uninstall agent --purge` |
| `status` | 查看各组件安装/运行状态 | `proxy-panel status` |
| `logs <component>` | 查看 journalctl 日志 | `sudo proxy-panel logs hub --lines 100 --follow` |
| `restart <component>` | 重启 systemd 服务 | `sudo proxy-panel restart agent` |

> 涉及系统变更的子命令（install / upgrade / rollback / uninstall / restart）需要 root 权限。

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

- **框架**: React 18 + TypeScript
- **构建工具**: Vite 6
- **UI 库**: HeroUI
- **样式**: Tailwind CSS v4
- **路由**: React Router v7
- **国际化**: react-i18next
- **HTTP 客户端**: Axios

### 项目结构

```
crates/pp-web/
├── src/
│   ├── main.tsx              # React 入口
│   ├── App.tsx               # 路由与布局
│   ├── api/                  # HTTP API 封装
│   │   └── client.ts
│   ├── context/              # React Context（Auth 等）
│   ├── components/           # 可复用组件与导航
│   ├── pages/                # 页面组件
│   │   ├── Dashboard.tsx
│   │   ├── Nodes.tsx
│   │   ├── Protocols.tsx
│   │   ├── Bindings.tsx
│   │   ├── Clients.tsx
│   │   ├── Subscriptions.tsx
│   │   └── ...
│   ├── i18n/                 # 翻译 JSON 文件
│   │   ├── zh-CN.json
│   │   └── en-US.json
│   └── index.css             # Tailwind CSS 入口
├── index.html
├── package.json
├── tsconfig.json
└── vite.config.ts
```

### 添加新页面

1. 在 `src/pages/` 创建新页面组件（如 `NewPage.tsx`）
2. 在 `src/components/nav.ts` 的 `navItems` 中添加导航项
3. 在 `src/App.tsx` 的路由配置中添加对应路由
4. 在 `src/i18n/zh-CN.json` 和 `src/i18n/en-US.json` 中添加翻译键
5. 如页面需要 API 调用，在 `src/api/client.ts` 中添加请求函数

### API 调用示例

```typescript
// src/api/client.ts
import { apiClient } from './client';

export interface Node {
  id: string;
  name: string;
  address: string;
}

export const listNodes = () => apiClient.get<Node[]>('/nodes');
export const createNode = (data: Partial<Node>) => apiClient.post<Node>('/nodes', data);
```

### 国际化

翻译文件位于 `src/i18n/zh-CN.json` 和 `src/i18n/en-US.json`。在组件中使用：

```tsx
import { useTranslation } from 'react-i18next';

export function MyPage() {
  const { t } = useTranslation();
  return <h1>{t('my-page.title')}</h1>;
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
