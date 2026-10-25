# ProxyPanel 开发指南

本文档面向项目开发者，描述开发环境搭建、常用任务、调试方法和代码贡献流程。

---

## 目录

1. [环境准备](#环境准备)
2. [项目初始化](#项目初始化)
3. [开发工作流](#开发工作流)
4. [数据库开发](#数据库开发)
5. [前端开发](#前端开发)
7. [测试](#测试)
8. [调试技巧](#调试技巧)
9. [代码审查清单](#代码审查清单)

---

## 环境准备

### 统一 Nix 开发环境

本地编译、运行、测试与提交统一进入仓库 `flake.nix` / `flake.lock` 环境。
不再安装宿主机 Bun/Protobuf 或通过 apt/brew 补齐构建系统库。
环境清单与 Nix 安装方法见 [Nix 开发环境](../nix-flake.md)；Rust 版本由
`rust-toolchain.toml` 经 flake 提供的 rustup 管理。

```bash
nix develop
# 自动化不加载用户 shell rc，防止用户 shell 覆盖 flake：
nix develop --command bash --noprofile --norc -c 'bun run --filter pp-web verify'
```

若 Nix 下载连接失败，可设置本机 HTTP 代理后重试（此工作区示例）：

```bash
export http_proxy="http://127.0.0.1:7899"
export https_proxy="http://127.0.0.1:7899"
nix develop
```

下文本地命令均假定已进入该环境。数据库由外部服务提供；客户端构建在独立 Lanpath 仓库执行。

---

## 项目初始化

### 1. 克隆仓库

```bash
git clone https://github.com/ybakiame/multi-proxy-panel.git
cd multi-proxy-panel
nix develop
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

# 5. 静态检查 + 测试门禁
bun run verify:rust        # = cargo fmt --check + workspace clippy + test
# 或快速版（跳过测试）：bun run verify:rust:fast

# 6. 静态检查
cargo clippy --workspace --all-targets -- -D warnings

# 7. 运行测试
cargo test --workspace

# 8. 提交代码
git add .
git commit -m "feat: your feature description"
```

### 变更工作流（Workflows）

本节定义仓库实际执行的变更工作流。每个工作流都必须覆盖：使用时机、上下文发现、计划、
实现、验证、文档、最终复查；完成定义与停止/升级条件统一见 `AGENTS.md` §11。

#### 功能开发（feature）

- **使用时机**：新增用户可见能力、接口、协议支持或数据模型。先判断是否触及架构边界
  （架构边界变更需先有 ADR，见 `docs/adr/README.md`）。
- **上下文发现**：读 `AGENTS.md` §1/§3/§4 与对应模块文档（`docs/architecture.md`、
  `docs/api_reference.md`），确认现有约定与可复用结构；查 `docs/adr/` 是否已有相关决策。
- **计划**：拆分为一个逻辑变更单元（大改动拆多个原子提交），列出受影响 crate / app、
  接口与数据变更、文档同步项（`AGENTS.md` §9）。
- **实现**：遵循 §3 代码规范与 §6 常见修改任务；数据库变更走 Migration + `UPGRADE_STEPS`
  （见 `.agents/rules/data-migration.md`）。
- **验证**：`cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo test --workspace`；涉及 Rust 提交前跑 `bun run verify:rust`；前端改动跑对应包
  `bun run --filter <pkg> verify`。
- **文档**：更新 §9 表格中受影响文档；用户可见变更写入 `CHANGELOG.md`。
- **最终复查**：`git diff` 逐项复查，确认无无关改动、无半成品；提交遵循 §5。

#### 缺陷修复（bugfix）

- **使用时机**：既有行为与预期/文档不符。
- **上下文发现**：复现问题，确定受影响版本、模块与触发条件；收集日志
  （见本文「调试技巧」）。
- **计划**：在不扩大改动面的前提下定位根因；若根因涉及架构缺陷，转 feature 并评估 ADR。
- **实现**：最小修复；禁止顺带重构无关代码或格式化无关文件（原子化提交）。
- **验证**：为缺陷补充回归测试（无法自动化的前端场景记录手动验证步骤）；运行受影响
  crate 测试与 CI 同款命令。
- **文档**：若行为或配置语义变化，同步 `docs/` 与 `CHANGELOG.md`（`### Fixed`）。
- **最终复查**：确认修复能通过原始复现步骤，且未掩盖相邻问题。

#### 重构（refactor）

- **使用时机**：不改变外部行为的结构调整（拆分文件、提取模块、去重）。
- **上下文发现**：确认现有测试覆盖范围作为行为基线；文件规模规则见
  `.agents/rules/code-organization.md`。
- **计划**：分步拆分，每步保持可编译；不扩大 `pub` 可见性。
- **实现**：纯结构变更，不夹带语义修改。
- **验证**：`cargo test --workspace`（或前端 `verify`）结果与重构前一致；纯重构必须
  行为不变。
- **文档**：仅当公开路径/模块结构变化时更新 `docs/architecture.md` 等文档。
- **最终复查**：确认 diff 中无语义变化；一个拆分一个提交。

#### 文档（docs）

- **使用时机**：仅修改说明性内容（README、docs/、AGENTS.md、CHANGELOG）。
- **上下文发现**：相关内容以**实现与已接受 ADR** 为准（`AGENTS.md` §11 停止条件）；
  采集代码/配置证据，避免凭印象改写。
- **计划**：列出涉及的文档、交叉引用与需要同步的索引（`docs/index.md`、`docs/adr/README.md`）。
- **实现**：保留既有项目知识，不整篇重写；不把厂商/代理特有内容写入项目契约。
- **验证**：校验文档内链接与相对路径可解析、命令真实存在（例如 `bun run verify:rust`、
  `scripts/check-rust-gates.sh`）；必要时抽检实现位置。
- **文档**：同步更新 `docs/index.md` 与 `AGENTS.md` §9 相关行。
- **最终复查**：确认没有引入未经验证的断言；一次逻辑变更一个提交。

#### 协议采纳（adoption）

- **使用时机**：首次采纳 Agent Development Protocol，或仓库发生重大变化后复跑采纳。
- **上下文发现**：只读发现仓库证据（结构、构建、验证、约定、代理组件、既有文档与 ADR）。
- **计划**：两遍制——Pass 1 只产出 `docs/adoption/project-inventory.json`、
  `protocol-mapping.json` 与审计报告；Pass 2 在人工确认开放决策后生成文档。
- **实现**：只生成有证据支撑的 ProxyPanel 文档；不拷贝 Protocol 文档，不改动子模块。
- **验证**：JSON 产物对照 `.protocol/agent-development-protocol/schemas/` 校验；执行
  链接/命令校验与适用项目验证；对照 `docs/conformance.md` 做符合性检查。
- **文档**：产出 `docs/adoption/adoption-report.json` 与 `adoption-report.md`，记录开放
  决策、接受的缺口与已知限制。
- **最终复查**：确认无未解决的实质矛盾；Pass 1 与 Pass 2 的写入边界各自成立。

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

前端依赖在**仓库根目录**统一安装（单一 `bun.lock`，Bun workspaces）：

```bash
# 仓库根目录一次安装所有前端依赖
bun install

# panel 管理系统（Vite 开发服务器，端口 5173）
bun run --filter pp-web dev
```

也可 `cd apps/panel && bun run dev`（依赖仍由根目录 `bun install` 提供）。

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

在每次 push 到 `main`/`master` 或提交 Pull Request 时触发；`changes` 按 Rust 与管理 Web
路径分别触发 `rust`、`web` 门禁。Nix 配置变更同时触发两侧。所有编译命令在项目 Nix 环境执行。

| Job | 验证范围 |
| --- | --- |
| `rust` | 根 workspace fmt、Clippy（`-D warnings`）、全部测试 |
| `web` | 根目录 frozen Bun 安装、`pp-web verify`（构建、oxlint、oxfmt） |

### Release (`.github/workflows/release.yml`)

推送 `v*` 标签或手动触发时执行，保留服务端发行链路：

1. `web`：构建管理 Web。
2. `build`：在既有 x86_64 / aarch64 Linux runner 编译 Hub、Agent、CLI，打包 tar.gz。
3. `release`：汇总服务端 tar.gz，生成 `SHA256SUMS` 并创建 GitHub Release。
4. `docker`：构建并发布 Hub / Agent GHCR 镜像。

客户端测试构建、Windows 安装包和 updater 由独立 [Lanpath 仓库](https://github.com/ybakiame/lanpath)
维护，未作为本仓的发布依赖。

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

- **框架**: React 19 + TypeScript
- **构建工具**: Vite 8
- **UI 组件**: shadcn/ui + Base UI（组件源码位于 `src/components/ui/`，通过 `components.json` 与 shadcn CLI 维护）
- **样式**: Tailwind CSS v4
- **路由**: React Router v7
- **国际化**: react-i18next
- **HTTP 客户端**: Axios
- **包管理器**: Bun workspaces（依赖在仓库根目录安装，单一 `bun.lock`）

### 项目结构

```
apps/panel/
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

**前端验证能力（当前边界）：**

管理 Web 没有单元测试运行器，自动化验证为构建 + oxlint + oxfmt：

```bash
bun run --filter pp-web verify
```

前端行为变更还需记录手动验证结果；新增自动化测试门禁仍需项目决策。

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

Hub 应用级 INFO/WARN/ERROR 事件异步写入系统日志；Agent 上报日志保存在节点日志。
管理页面可切换两类记录。RUST_LOG 同时控制终端和采集入口，Hub 不将 SQL/HTTP DEBUG
事件写入数据库；终端详细调试输出仍可能包含敏感内容。首次修复升级需同时发布 Hub 与
管理 Web，旧 Hub 不支持 `kind=agent` 查询。

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

提交 PR 前，请确认以下事项（与 `AGENTS.md` §11 完成定义一致）：

### 功能性

- [ ] 新功能有对应的测试覆盖；前端改动至少执行对应包 `verify` 并记录手动验证步骤
- [ ] 手动测试通过（至少运行一次完整流程）
- [ ] 错误路径已处理（如数据库连接失败、网络超时）

### 代码质量

- [ ] `cargo fmt --all --check` 已执行
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 无警告
- [ ] `cargo test --workspace` 全部通过
- [ ] 涉及 Rust 时 `bun run verify:rust` 通过
- [ ] 无裸 `unwrap()` / `expect()`（初始化代码除外）
- [ ] 新增公开的 API 有文档注释 (`///`)

### 安全性

- [ ] 用户输入已验证和清理
- [ ] 无硬编码密钥或密码
- [ ] 数据库查询无 SQL 注入风险（使用 Sea-ORM 参数绑定）

### 文档

- [ ] README.md 已更新（如添加新功能或变更使用方式）
- [ ] AGENTS.md 已更新（如变更架构或规范）
- [ ] `docs/` 下相关文档已更新（含 `docs/index.md` / `docs/adr/README.md` 索引）
- [ ] `CHANGELOG.md` 已记录用户可见变更

---

## 常见问题

### Q: 编译失败，提示 protobuf 相关错误？

在仓库根目录进入 `nix develop`，然后执行 `protoc --version`。
protobuf 编译器由 flake 提供，无需安装宿主机包。

### Q: Web 前端编译后无法访问？

确保 Hub 的 `--static-dir` 指向正确路径：

```bash
cargo run --bin proxy-panel-hub -- --static-dir apps/panel/dist
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
