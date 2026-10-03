# 编码代理适配器（Agent Adapters）

本文件记录 ProxyPanel 仓库中被使用的**具体编码代理**及其适配层。它是 Agent Adapter
文档：厂商特有的模型名、权限、子代理工具与命令语法只写在这里，**不进入项目契约**
（`AGENTS.md`、`README.md`、`docs/architecture.md` 等保持代理无关）。

项目契约对“做什么、做到什么程度”负责；本目录只说明“由哪个代理、用什么机制执行”。

---

## 1. 规则优先级

同一仓库同时存在多层代理指令，冲突时按以下顺序取权威（上 > 下）：

1. **项目契约**：`AGENTS.md`、`README.md`、`docs/`、`docs/adr/`、`docs/adr/README.md`。
2. **项目级代理规则**：`.agents/rules/*.md`（由 `AGENTS.md` §2.4 引用，属于项目规则）。
3. **厂商适配层**：`opencode.jsonc` + `.opencode/**`、`.pi/**`、`apps/panel/AGENTS.md`。
4. **代理/框架默认行为**（未在本仓库显式配置时）。

适配层不得覆盖或放宽第 1、2 层规则；若适配层与项目契约冲突，以项目契约为准并修复
适配层（参见 `AGENTS.md` §11 停止/升级条件）。

---

## 2. opencode 适配器

| 项 | 内容 |
|----|------|
| 配置入口 | `opencode.jsonc` |
| 代理指令 | `.opencode/AGENTS.md` |
| 代理定义 | `.opencode/agents/orchestrator.md`、`executor.md`、`reviewer.md` |
| 验证命令矩阵 | `.opencode/rules/coding-standards.md` |

**形态**：三层委派——`orchestrator`（primary，规划 + 人机确认）→ `executor`
（subagent，读写代码并跑验证）→ `reviewer`（subagent，只读评审）。
`opencode.jsonc` 中 `default_agent = "orchestrator"`，并配置了模型与权限：

- orchestrator：`edit: ask`；`bash` 默认 `deny`，仅放行 `grep/rg/git status/git log/git diff/git show`；`task` 仅允许 `executor`、`reviewer`。
- executor：`edit: allow`、`bash: allow`、`task: deny`（不可再委派）。
- reviewer：`edit: deny`、`bash: allow`、`task: deny`。

**能力边界**：orchestrator 不直接改代码（权限为 ask 且 bash 受限）；执行与评审分离，
评审角色无写权限。

**验证**：`.opencode/rules/coding-standards.md` 给出 Rust（根 workspace）、客户端壳
（`apps/client/src-tauri`，独立 cargo 项目）与前端（`apps/client` / `apps/panel` /
`packages/client-core`）的验证命令矩阵，并指向 `AGENTS.md` §5 提交规范。

**已知限制**：

- 模型名与 provider 组合（如 `kimi-for-coding/k3`、`deepseek/deepseek-v4-flash`）是本地
  环境相关配置，换环境需自行调整。
- reviewer 无写权限，评审结论必须回传 orchestrator 后再由 executor 修改。
- `.opencode/AGENTS.md` 只做分层策略说明并指向本文件，不重复项目契约。

---

## 3. pi 适配器

| 项 | 内容 |
|----|------|
| 主代理提示词 | `.pi/master.md` |
| 子代理定义 | `.pi/agents/coder.md`、`reviewer.md`、`writer.md` |
| 模型映射 | `.pi/subagent-isolation.json` |
| 技能 | `.pi/skills/`（brainstorming / systematic-debugging / writing-clearly-and-concisely） |

**形态**：主 agent 是任务指挥官与质量守门员，**不编辑代码、不执行命令、不写文件**，
一切通过 `subagent` 工具委派；每个子 agent 运行在独立进程，拥有独立 system prompt 与
skills，上下文与主 agent 隔离。

**子代理职责与工具**：

| 子代理 | 工具 | 定位 |
|--------|------|------|
| `coder` | read / write / edit / bash / grep / find / ls | 实现与验证 |
| `reviewer` | read / grep / find / ls | 只读评审，输出可操作反馈 |
| `writer` | read / write / edit / grep / find / ls | 文档、README、提交信息 |

**委派契约**：每次委派必须包含背景 / 输入 / 要求 / 输出格式 / 验收标准五段；验收标准
必须含验证命令与输出。未达标结果由主 agent 打回重做，不直接交付。

**模型映射**（`.pi/subagent-isolation.json`）：coder / writer 用
`deepseek/deepseek-flash`（thinking high），reviewer 用 `kimi-coding/k3-256k`（thinking low）。

**已知限制**：

- 主 agent 无写入与命令权限，必须在委派上下文中提供充分素材，否则子代理无法自行补足。
- `.pi/plans/` 是历史计划记录（例如 2026-10-01 桌面同步计划中引用的 `apps/desktop`），
  按历史材料保留，不回溯改写；当前路径以 `docs/architecture.md` 与 ADR-0007 为准。

---

## 4. `.agents/` 项目级规则与技能

| 项 | 内容 |
|----|------|
| 规则 | `.agents/rules/code-organization.md`、`data-migration.md`、`package-manager.md`、`client-state-management.md` |
| 技能 | `.agents/skills/heroui-react`、`heroui-native`、`heroui-migration`，锁文件 `skills-lock.json` |

`.agents/rules/` 是**项目级**代理规则（不是厂商配置）：`AGENTS.md` §2.4 直接引用
`code-organization.md` 的文件规模阈值，husky pre-commit 的 `scripts/check-file-size.sh`
实现同一门禁。

**已知限制**：

- `.agents/rules/client-state-management.md` 标题与正文按 `apps/client`（原
  `apps/desktop`）编写；随 ADR-0007 已修正为 `apps/client` 语义。
- `.agents/skills/heroui-*` 是第三方（HeroUI）技能包，随 `skills-lock.json` 锁定版本；
  其内容不属于项目契约。

---

## 5. 嵌套 `apps/panel/AGENTS.md`

`apps/panel/AGENTS.md` 是 HeroUI 官方工具生成的**组件文档索引**（`HEROUI-REACT-AGENTS-MD`
区块），只作用于 `apps/panel` 子树，用于让代理在写 HeroUI 代码前查最新组件文档。
它不重复根契约规则；根契约始终优先。

---

## 6. 适配器符合性状态

按协议对 Agent Adapter 的符合性要求（能发现规则、能执行工作流、能执行或委派验证、
能暴露失败与阻塞、能满足完成定义），当前证据状态：

| 检查项 | opencode | pi | `.agents/` 规则 |
|--------|:--------:|:--:|:---------------:|
| 仓库规则可发现（指向 `AGENTS.md` / `docs/`） | ✅（`.opencode/AGENTS.md`） | ✅（`.pi/master.md`） | ✅（`AGENTS.md` §2.4 引用） |
| 验证命令有明确出处 | ✅（`.opencode/rules/coding-standards.md`） | ✅（子代理验收标准要求验证命令） | ✅（`scripts/check-file-size.sh`） |
| 失败/阻塞可上浮 | ✅（orchestrator 判断 + 人机确认） | ✅（主 agent 质量门禁与重派） | 不适用（规则文件无执行体） |
| 执行记录（代表性任务） | ⏳ 未在仓库中留档 | ⏳ 未在仓库中留档 | 不适用 |

**已知缺口（接受）**：仓库目前没有留存“代表性任务执行记录”（如某次 feature/bugfix
由某适配器执行并验证的记录）。这是接受中的缺口，见
`docs/adoption/adoption-report.md`；补齐方式是在后续真实任务中把验证命令与结果附到 PR
或 `docs/adoption/`。

---

## 7. 维护约定

- 新增或调整编码代理：更新本文件对应小节 + `AGENTS.md` §9 同步表。
- 路径迁移（如 ADR-0007 的 `apps/desktop` → `apps/client`）后，运行
  `grep -rn "apps/desktop\|apps/mobile" .opencode .agents .pi opencode.jsonc` 修复适配层；
  `.pi/plans/` 等历史材料除外。
- 不要把厂商模型名、权限配置、skill 版本写入 `AGENTS.md` / `README.md` / `docs/architecture.md`。
