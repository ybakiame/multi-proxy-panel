# Protocol Adoption Report — ProxyPanel

## Project

- **Project:** ProxyPanel
- **Repository:** https://github.com/ybakiame/multi-proxy-panel
- **Adoption date:** 2026-10-04
- **Protocol version:** 0.1.0 (`.protocol/agent-development-protocol/protocol-manifest.json`)
- **Protocol submodule revision:** `310fba5baa384c912e1fd8300e33d72a057908c5`
- **Base revision:** `8ebba040e8197f2428a4b0929359507aed0b525d`；Pass 2 改动在其上拆为 8 个原子提交
  （`5a81a7d` → `b770602`，见「Applied Commits」）
- **Pass 1 records:** [project-inventory.json](project-inventory.json)、[protocol-mapping.json](protocol-mapping.json)、[audit-pass1.md](audit-pass1.md)

## Summary

- **Adoption status: `partial`**（按 `adoption-report.schema.json` 的状态词表）
- **Overall notes：** Pass 2 按已批准的 ADC-001…ADC-008 决策生成/修复仅属于 ProxyPanel 的文档：
  修正了 Pass 1 登记的全部 10 项经确证的矛盾，新增 ADR 约定与索引、编码代理适配器文档、
  变更工作流、完成定义与升级条件、`CHANGELOG.md`，并补齐 `LICENSE`（ADC-008，解决
  `README.md → LICENSE` 断链）；Protocol 文档未被拷贝，子模块未被改动。
  仍有一处**适用缺口**，故不声明 clean：agent-adapter 缺少代表性任务执行记录
  （已在适配器文档中显式接受并标注为缺口）。

- **Capability summary（Pass 2 后）：** Existing 6 · Partial 1 · Missing 0 · N/A 6（共 13）。

## Evidence Reviewed

- **Repository structure:** `crates/`（13 个 workspace 成员）、`apps/{panel,client}`、`packages/client-core`、`proto/`、`scripts/`、`deploy/`、`docs/`。
- **Configuration:** `Cargo.toml`、`package.json`、`bun.lock`、`rust-toolchain.toml`、`flake.nix`、`docker-compose.yml`、`Dockerfile.*`、`config/`。
- **Tests/validation:** 83 个含 `#[cfg(test)]` 的 Rust 源文件、`crates/pp-client/tests/real_core_e2e.rs`、`.husky/pre-commit`、`scripts/check-rust-gates.sh`、`scripts/check-file-size.sh`。
- **CI/CD:** `.github/workflows/ci.yml`、`.github/workflows/release.yml`。
- **Existing documentation:** `README.md`、`docs/index.md`、`docs/architecture.md`、`docs/development.md`、`docs/deployment.md`、`docs/api_reference.md`、`docs/contributing.md`、`nix-flake.md`。
- **Existing Agent/Tool/Prompt/Memory/MCP components:** `opencode.jsonc`、`.opencode/**`、`.pi/**`、`.agents/rules/**`、`.agents/skills/**`、`skills-lock.json`、`apps/panel/AGENTS.md`。（结论：均为编码代理适配器资产，非产品 AI Agent 子系统。）
- **ADRs:** `docs/adr/0001`–`0007` 及 `docs/adr/README.md`（本次新增）。
- **Protocol reference:** `.protocol/agent-development-protocol/`（`ADOPT.md`、`protocol-manifest.json`、`docs/adoption/**`、`docs/conformance.md`、`schemas/**`）。

## Capability Mapping

Pass 1 的分类记录在 `protocol-mapping.json`（规划记录，保持冻结）；下表给出 Pass 2 后的结果。

| Capability | Pass 1 | Pass 2 | 证据 / 动作 |
|---|---|---|---|
| Project Contract | Partial | **Existing** | `AGENTS.md`（含 §10 工作流、§11 DoD/升级条件）、`README.md`、`docs/index.md` 已对齐；ADC-008 补齐 `LICENSE`，许可证链接已解析 |
| Architecture | Partial | **Existing** | `docs/architecture.md` 客户端章节改为 `apps/client`（ADR-0007）、技术栈版本修正、`AppState`/proto 说明与实现对齐 |
| Workflows | Partial | **Existing** | `docs/development.md#变更工作流workflows`：feature / bugfix / refactor / docs / adoption 五个适用工作流，含全部阶段；DoD 与停止条件见 `AGENTS.md` §11 |
| Testing | Partial | **Existing** | 真实测试面已记录：单元测试、`real_core_e2e`（`--include-ignored`）、DB/gRPC 指引、前端 `verify` 边界、CI 门禁；前端无测试运行器为已接受的现状（ADC-005） |
| Evaluation | N/A | **N/A** | 无 AI/LLM Agent 运行时；`docs/plans/2026-10-03-client-merge-evaluation.md` 属架构评估（ADC-002） |
| Operations | Existing | **Existing** | `docs/deployment.md`、Docker/compose、安装脚本、CLI 生命周期、release CI（未改动，抽查通过） |
| Agents | N/A | **N/A** | `pp-agent` 是代理节点守护进程，不是 AI Agent（ADC-002） |
| Tools | N/A | **N/A** | 无项目自有 Agent Tool 契约（ADC-002） |
| Prompts | N/A | **N/A** | 编码代理提示词属于适配器资产，记录于 `docs/adapters/README.md`（ADC-002） |
| Memory | N/A | **N/A** | 仓库内“状态”是应用状态与产品持久化（ADC-002） |
| MCP | N/A | **N/A** | 全仓无 MCP 集成（ADC-002） |
| ADRs | Partial | **Existing** | 新增 `docs/adr/README.md`（状态约定/索引/模板）；修复确证元数据漂移；历史正文保留 |
| Agent Adapter | Partial | **Partial** | 新增 `docs/adapters/README.md`、规则优先级、验证矩阵与适配器路径修复；缺口：无代表性任务执行记录（接受并延后） |

## Contradiction Resolution

Pass 1 登记的 10 项矛盾处理结果（全部按 ADC-003 的权威来源：实现 + 已接受 ADR）：

| ID | 处理 | 落点 |
|----|------|------|
| C-01 | 已修复 | `docs/architecture.md` 客户端章节与整体架构图、`README.md` 结构树、`Cargo.toml` 成员注释、`.opencode/rules/coding-standards.md`、`.agents/rules/client-state-management.md`、`nix-flake.md`（`apps/desktop`/`apps/mobile` → `apps/client`） |
| C-02 | 已修复 | `README.md` 结构树与 crate 表/架构正文一致 |
| C-03 | 已修复 | `AGENTS.md` §7 与 `docs/development.md`「测试」记录 `crates/pp-client/tests/real_core_e2e.rs` |
| C-04 | 已修复 | `docs/index.md`、`docs/architecture.md`、`docs/development.md` → React 19 / Vite 8 / HeroUI 3 |
| C-05 | 已修复 | `AGENTS.md` §4.1 `AppState` 字段与 `crates/pp-hub/src/state.rs` 对齐 |
| C-06 | 已修复 | `AGENTS.md` §3.6、`README.md` 结构树列出 `singbox_daemon.proto` |
| C-07 | 已修复 | 新增 `CHANGELOG.md`；`docs/contributing.md` 发布步骤与文档表更新 |
| C-08 | 已修复（仅元数据） | ADR-0002 `Status: Accepted`；ADR-0006 `Date: 2026-10-02`（依据首次提交日期）；审批与溯源见 `docs/adr/README.md#元数据修正记录` |
| C-09 | 已修复 | `README.md`、`docs/index.md`、`docs/development.md` 统一为仓库根目录 `bun install`（单一 `bun.lock`） |
| C-10 | 已修复 | `.opencode/AGENTS.md` 占位符替换为真实栈与优先级指向；`docs/adapters/README.md` §1 明确规则优先级 |

新增修复（Pass 1 未登记，协调阶段发现）：`crates/pp-agent/README.md`、`crates/pp-hub/README.md`
的相对链接指向 `../deploy`、`../docs`（应为 `../../`）；`nix-flake.md` 引用了不存在的
`apps/mobile/scripts/android-ndk-env.sh` 与已不存在的 Dioxus/KMP ADR。

## Decisions Applied

| ID | 决策 | 落地 |
|----|------|------|
| ADC-001 | 保留既有文档约定 | 未创建 `docs/workflows/`、`docs/testing/`、`docs/decisions/`、`docs/architecture/`；工作流/测试内容写入 `docs/development.md`，ADR 约定写入 `docs/adr/README.md` |
| ADC-002 | 编码代理基础设施属 Adapter 范围 | 新增 `docs/adapters/README.md`；未创建 agents/tools/prompts/memory/mcp/evaluation 子系统；项目契约中无厂商配置（已自动校验） |
| ADC-003 | ADR-0007 + 实现为准 | 修正全部当前文档；ADR-0003 正文与状态未改（历史保留） |
| ADC-004 | 入口已提交、子模块不动 | 入口位于 `8ebba04`；子模块 `310fba5` 清洁未改；未拷贝 Protocol 文件 |
| ADC-005 | 不新增强制前端测试门禁 | `AGENTS.md` §7 与 `docs/development.md` 如实记录前端验证能力（构建 + oxlint + oxfmt） |
| ADC-006 | 保留 `docs/adr/` 并补约定 | `docs/adr/README.md`（状态词表、必备字段、索引、模板）+ 仅元数据修复 |
| ADC-007 | 新增 `CHANGELOG.md` | 创建 Keep a Changelog 风格文件：`[Unreleased]` 记录 v0.4.5 之后的 mainline 工作，v0.4.0–v0.4.5 条目依据 git 发布标签（日期 + 标签说明）重建；接入 `README.md`、`docs/index.md`、`docs/contributing.md`、`AGENTS.md` §9 |
| ADC-008 | 补齐 `LICENSE` | 新增 GNU AGPL-3.0 完整文本（235 行 / 34020 字节，与 `/usr/share/licenses/spdx/AGPL-3.0-or-later.txt` md5 一致 `216109e2…`），对应 `Cargo.toml` / `package.json` 已声明的 `AGPL-3.0-or-later`；`README.md` badge 与许可证链接解析通过 |

## Applied Commits

Pass 2 改动按 `AGENTS.md` §5 拆为 8 个原子提交（在 `8ebba04` 之上）：

| Commit | 内容 |
|--------|------|
| `5a81a7d` | ADR 约定、索引与模板；ADR-0002 / ADR-0006 元数据修复 |
| `a71a1dc` | 编码代理适配器文档；适配器过期路径修复 |
| `efec011` | `LICENSE`（AGPL-3.0-or-later，ADC-008） |
| `422a72a` | `CHANGELOG.md`；`docs/contributing.md` 文档表与发布步骤 |
| `7db8c49` | Pass 1 审计产物（inventory / mapping / audit） |
| `7f7e3c3` | Pass 2 采纳报告（本文件与 JSON） |
| `d22f9a1` | `AGENTS.md` 工作流/DoD/升级条件与契约漂移修复；`docs/development.md` |
| `b770602` | 文档对齐 `apps/client`（README / index / architecture / Cargo.toml / crate README / nix-flake） |

## Files Created

- [`CHANGELOG.md`](../../CHANGELOG.md)
- [`LICENSE`](../../LICENSE)（GNU AGPL-3.0 完整文本）
- [`docs/adr/README.md`](../adr/README.md)
- [`docs/adapters/README.md`](../adapters/README.md)
- [`docs/adoption/adoption-report.json`](adoption-report.json)
- [`docs/adoption/adoption-report.md`](adoption-report.md)（本文件）

## Files Updated

- [`AGENTS.md`](../../AGENTS.md)：proto 清单、`AppState`、测试策略、文档同步表、§10 工作流、§11 完成定义与升级条件、§12 资源。
- [`README.md`](../../README.md)：apps 结构树、proto 结构树、Bun workspaces 安装约定、文档索引与 CHANGELOG 链接。
- [`docs/index.md`](../index.md)：客户端条目、技术栈版本、ADR/适配器/采纳记录/Nix/CHANGELOG 入口、安装约定。
- [`docs/architecture.md`](../architecture.md)：客户端章节与整体架构图改写为 `apps/client`，MITM 作用域说明，技术栈版本。
- [`docs/development.md`](../development.md)：技术栈版本、根目录安装约定、变更工作流章节、测试面（e2e + 前端边界）、代码审查清单。
- [`docs/contributing.md`](../contributing.md)：文档位置表、发布步骤（`[Unreleased]` → 版本标题）。
- [`docs/adr/0002-client-rule-management.md`](../adr/0002-client-rule-management.md)（仅 `Status`）、[`docs/adr/0006-client-traffic-stats.md`](../adr/0006-client-traffic-stats.md)（仅 `Date`）。
- [`.opencode/AGENTS.md`](../../.opencode/AGENTS.md)、[`.opencode/rules/coding-standards.md`](../../.opencode/rules/coding-standards.md)、[`.agents/rules/client-state-management.md`](../../.agents/rules/client-state-management.md)（适配器路径与占位符修复）。
- [`crates/pp-agent/README.md`](../../crates/pp-agent/README.md)、[`crates/pp-hub/README.md`](../../crates/pp-hub/README.md)（相对链接）。
- [`Cargo.toml`](../../Cargo.toml)（成员注释，无构建语义）、[`nix-flake.md`](../../nix-flake.md)（NDK 脚本路径与历史说明）。

## Files Preserved

- `docs/adr/0001`、`0003`、`0004`、`0005`、`0007`（历史决策内容与状态未改）。
- `docs/plans/**`、`docs/research/**`、`.pi/plans/**`（带日期的历史记录）。
- `docs/deployment.md`、`docs/api_reference.md`（复核准确，未改动）。
- `opencode.jsonc`、`.opencode/agents/**`、`.pi/**`、`.agents/skills/**`、`skills-lock.json`、`apps/panel/AGENTS.md`（厂商适配资产）。
- `.protocol/agent-development-protocol/**`（子模块未改）。
- `docs/adoption/project-inventory.json`、`protocol-mapping.json`、`audit-pass1.md`（Pass 1 证据/规划记录，冻结）。

## Intentionally Omitted / N/A

- `docs/workflows/`、`docs/testing/`、`docs/decisions/`、`docs/architecture/`：与既有约定重复（ADC-001），不迁移、不新建。
- `docs/evaluation/`、`docs/agents/`、`docs/tools/`、`docs/prompts/`、`docs/memory/`、`docs/mcp/`：能力 N/A（ADC-002），不生成空占位。

## Unresolved Decisions

- 无。原 ADC-008（`README.md` → 不存在的 `LICENSE`）已在本次通过新增 `LICENSE`
  （GNU AGPL-3.0 完整文本，对应已声明的 `AGPL-3.0-or-later`）解决；未新增许可证政策，
  仅补齐此前缺失的文本文件。此前为不静默解决未决政策问题而暂缓的内容现已按用户批准执行。

## Validation

- **Structural：** `project-inventory.json`、`protocol-mapping.json`、`adoption-report.json`
  均通过 `.protocol/agent-development-protocol/schemas/` 下的 JSON Schema（ajv 2020-12）。
  Mapping：13 个能力、各恰一个状态、非 N/A 均有证据、N/A 均有适用性理由。
- **Content：** Pass 1 的 92 条证据路径全部复核存在；新增/修改文档的每条主张均以当前实现、
  配置、ADR 与 git 历史为依据；历史材料未被改写。
- **Links：** 177 个 Markdown 文件、1034 条相对链接、0 个未解析锚点、0 条断链
  （ADC-008 补齐 `LICENSE` 后 `README.md → LICENSE` 已解析）。
- **Commands：** 52 条文档化命令断言（cargo bin/test 目标经 `cargo metadata`、bun workspace
  脚本、`scripts/*.sh` 路径）全部解析通过，0 问题。
- **Project validation：** 变更集为文档 + 一处 `Cargo.toml` 注释（无构建语义）+ 新增 `LICENSE`
  （纯文本，不参与构建）。`git diff --check` 通过；`scripts/check-file-size.sh` 对所有改动文件
  退出码 0；`bun run verify:rust:fast` 通过（fmt --check + 根 workspace clippy + 客户端壳
  host clippy + `aarch64-linux-android` 检查）；完整 `bun run verify:rust` 通过
  （含 `cargo test --workspace`）。
- **Protocol conformance：** 21/21 检查通过（必备契约文件、文档索引、架构文档、适用工作流覆盖、
  完成定义、升级条件、适配器文档与规则优先级、项目契约无厂商配置、采纳记录齐备、子模块未改动）；
  对照 `.protocol/agent-development-protocol/docs/conformance.md` 执行。

## Known Limitations

1. **agent-adapter 仍为 Partial：** 仓库没有 opencode/pi 适配器的代表性任务执行记录
   （在 `docs/adapters/README.md` §6 显式接受并标注）。
2. **前端无自动化测试：** `apps/panel`、`apps/client`、`packages/client-core` 的 `verify` 只是
   构建 + oxlint + oxfmt（ADC-005 接受，如实记录未加门禁）。
3. **历史路径保留：** ADR-0003/0004/0005/0006 正文与 Scope、`docs/plans/**`、`docs/research/**`、
   `.pi/plans/**` 中的 `apps/desktop`/`apps/mobile` 属当时真实记录，不回溯改写。
4. **ADR 元数据修复属事后更正：** ADR-0002 状态与 ADR-0006 日期依据实现证据与首次提交日期修正，
   溯源记录在 `docs/adr/README.md`；决策正文未改。
5. **提交范围：** Pass 2 改动已按原子提交拆分（`5a81a7d`…`b770602`，共 8 个）；本报告的
   提交范围字段在提交后随本次元数据更新同步（见「Applied Commits」）。
6. **项目验证面向文档/许可证变更：** 已跑通快速与完整 Rust 门禁，但本变更不涉及运行时行为。
7. **CHANGELOG 历史未完全回溯：** `v0.4.0` 之前的版本历史未在仓库保留，不做重建；
   `v0.4.0`–`v0.4.5` 条目依据 git 发布标签（日期 + 标签说明）整理；`[Unreleased]` 记录
   `v0.4.5` 之后 mainline 的主要主题（非逐提交，完整列表见 `git log v0.4.5..HEAD`）。

## Final Assessment

- **Conformance status：** `partial` —— 13 个能力中 6 个 Existing、6 个 N/A、1 个 Partial
  （agent-adapter 因缺少代表性执行记录）。无 Missing。唯一剩余缺口为已接受、已文档化的
  adapter 执行证据，不影响项目契约的准确性。
- **Follow-up actions：**
  1. 在后续真实任务中留存适配器执行证据（PR 描述或 `docs/adoption/`），以关闭 agent-adapter 缺口；
  2. 仓库发生重大变化后按 `docs/development.md#变更工作流workflows` 的 adoption 工作流复跑协调与校验。
