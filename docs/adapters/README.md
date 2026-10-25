# 编码代理适配器（Agent Adapters）

本文件记录 ProxyPanel 仓库中配置的**具体编码代理**及其适配层。它是 Agent Adapter
文档：厂商特有的模型名、权限、子代理工具与命令语法只写在这里，**不进入项目契约**
（`AGENTS.md`、`README.md`、`docs/architecture.md` 等保持代理无关）。

项目契约对“做什么、做到什么程度”负责；本目录只说明“由哪个代理、用什么机制执行”。

---

## 1. 规则优先级

同一仓库同时存在多层代理指令，冲突时按以下顺序取权威（上 > 下）：

1. **项目契约**：`AGENTS.md`、`README.md`、`docs/`、`docs/adr/`、`docs/adr/README.md`。
2. **项目级代理规则**：`.agents/rules/*.md`（由 `AGENTS.md` §2.4 引用，属于项目规则）。
3. **厂商适配层**：`.pi/**`、`apps/panel/AGENTS.md`。
4. **代理/框架默认行为**（未在本仓库显式配置时）。

适配层不得覆盖或放宽第 1、2 层规则；若适配层与项目契约冲突，以项目契约为准并修复
适配层（参见 `AGENTS.md` §11 停止/升级条件）。

---

## 2. pi 适配器

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
- 客户端专属历史计划已随拆仓移除；当前项目范围以 `docs/architecture.md` 与 ADR-0015 为准。

---

## 3. `.agents/` 项目级规则与技能

| 项 | 内容 |
|----|------|
| 规则 | `.agents/rules/code-organization.md`、`data-migration.md`、`package-manager.md` |
| 技能 | 当前无仓库锁定的第三方技能；`skills-lock.json` 保留空索引 |

`.agents/rules/` 是**项目级**代理规则（不是厂商配置）：`AGENTS.md` §2.4 直接引用
`code-organization.md` 的文件规模阈值，husky pre-commit 的 `scripts/check-file-size.sh`
实现同一门禁。HeroUI 技能已随前端迁移移除。

---

## 4. 嵌套 `apps/panel/AGENTS.md`

`apps/panel/AGENTS.md` 记录 shadcn/ui（Base UI）组件源码、CLI 与 Tailwind v4 的本地约定，
只作用于 `apps/panel` 子树。它不重复根契约规则；根契约始终优先。

---

## 5. 适配器符合性状态

按协议对 Agent Adapter 的符合性要求（能发现规则、能执行工作流、能执行或委派验证、
能暴露失败与阻塞、能满足完成定义），当前证据状态：

| 检查项 | pi | `.agents/` 规则 |
|--------|:--:|:---------------:|
| 仓库规则可发现（指向 `AGENTS.md` / `docs/`） | ✅（`.pi/master.md`） | ✅（`AGENTS.md` §2.4 引用） |
| 验证命令有明确出处 | ✅（子代理验收标准要求验证命令） | ✅（`scripts/check-file-size.sh`） |
| 失败/阻塞可上浮 | ✅（主 agent 质量门禁与重派） | 不适用（规则文件无执行体） |
| 执行记录（代表性任务） | ⏳ 未在仓库中留档 | 不适用 |

**已知缺口（接受）**：仓库目前没有留存“代表性任务执行记录”（如某次 feature/bugfix
由某适配器执行并验证的记录）。这是接受中的缺口，见
`docs/adoption/adoption-report.md`；补齐方式是在后续真实任务中把验证命令与结果附到 PR
或 `docs/adoption/`。

---

## 6. 维护约定

- 新增或调整编码代理：更新本文件对应小节 + `AGENTS.md` §9 同步表。
- 仓库结构调整后，运行
  `rg -n "cargo|bun|nix" .agents/rules .pi` 核对适配层命令与项目契约。
- 不要把厂商模型名、权限配置、skill 版本写入 `AGENTS.md` / `README.md` / `docs/architecture.md`。
