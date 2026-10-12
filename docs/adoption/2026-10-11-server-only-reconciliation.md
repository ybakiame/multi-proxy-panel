# 2026-10-11 ProxyPanel 服务端范围复核

维护者已确认 Lanpath 迁移验收完成，并授权原仓客户端完整清理。
本次按 [ADR-0015](../adr/0015-lanpath-independent-repository.md) 收窄工作区，
依据根 [AGENTS.md](../../AGENTS.md)、[架构](../architecture.md) 与实现核对。

## 当前归属

- Cargo workspace：`pp-common`、`pp-db`、`pp-proto`、`pp-config`、`pp-subscription`、
  `pp-core`、`pp-hub`、`pp-agent`、`pp-cli`，共 9 个 crate。
- Bun workspace：仅 `apps/panel`（`pp-web`），保留根目录单一 Bun 锁文件。
- 保留 Hub/Agent gRPC、公开订阅、数据库、CLI、管理 Web 与服务端 sing-box/mihomo 支持。
- 移除客户端应用、Go/Kotlin/Tauri/UI、客户端专属 4 个 crate、共享前端包、测试构建、
  updater、专属 ADR/计划/验证文档与本地客户端缓存；当前文档不含旧客户端开发步骤。
- 两仓平级维护，不新增客户端 submodule、路径或 Git 构建依赖。

Nix 环境移除 Go/JDK/SDK/NDK/Gradle/GUI 库；Linux CI 仅验证服务端 Rust 和管理 Web。
原有服务端 GNU/Linux Release、容器和 tar.gz 文件名保持兼容。
客户端签名文件保留在 Lanpath 本地 `.local/signing/`，只读归属缓存移到其忽略的 `target/`，
均不纳入提交。原仓 Cargo 缓存已完整清空后重建。用户数据、数据库与证书不作删除。

## 协议记录

协议子模块仍固定 `310fba5baa384c912e1fd8300e33d72a057908c5`，未改动子模块。
现有 inventory / mapping / adoption report 的原审计内容保留为历史证据，
新增 `maintenance_2026_10_11` 记录当前范围；历史删除路径链接固定到迁移前 Git 修订。
客户端专属 ADR 正文保留在 Git 历史和 Lanpath，当前 ADR 索引只包含服务端及仓库边界决策。

13 个协议能力的适用性没有变化：project-contract、architecture、workflows、testing、
operations、adrs 为 Existing；agent-adapter 为 Partial（已有适配器执行证据缺口继续保留）；
AI Agent 运行时相关 agents、evaluation、tools、prompts、memory、mcp 为 N/A。
管理 Web 未引入单元测试运行器，自动化边界仍为 build + oxlint + oxfmt。

## 验证

所有本地构建、运行和验证均在项目 Nix 环境执行。

| 检查 | 结果 |
| --- | --- |
| 清空旧 Cargo 缓存后 `cargo build --workspace --locked` | 通过 |
| 最终 `bun run verify:rust`（fmt / 严格 Clippy / 全 workspace 测试） | 134 passed，0 failed，0 ignored |
| 冻结 Bun 安装与 `bun run --filter pp-web verify` | 通过，构建 / lint / format 全部成功 |
| Hub、Agent、CLI `--help` 运行冒烟 | 通过 |
| 三份协议 JSON Schema、当前 9 个 crate / 唯一 Bun workspace | 通过 |
| 两份工作流依赖、两份 Docker workspace COPY 清单 | 通过 |
| 38 份项目 Markdown、120 个相对链接/锚点 | 通过 |
| Nix aarch64 环境求值、脚本语法、文件规模、`git diff --check` | 通过 |
| Cargo 剩余包版本与迁移前锁文件比对 | 无新增版本或无关升级 |
| 协议 gitlink 与子模块工作区 | 修订不变、工作区干净 |

未执行远端 CI、Docker 镜像构建或正式发布；两份 Dockerfile 的构建输入已静态核对，
服务端 Release 保留原有构建和部署契约。本轮没有修改服务端 API、数据库或核心运行行为，
仅移除无人调用的客户端专属接口；现有 Agent 生命周期回归仍通过。

清理后的应用目录只有 `apps/panel`，客户端专属目录、锁文件条目、Nix 输入和自动化引用均不存在。
签名资产保留在 Lanpath 本地忽略目录，Lanpath 跟踪工作区仍然干净。
2026-10-12 维护者授权将本轮清理作为一个原子提交保存在本地；未推送远端。
