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
3. **局部适配层**：`apps/panel/AGENTS.md`。
4. **代理/框架默认行为**（未在本仓库显式配置时）。

适配层不得覆盖或放宽第 1、2 层规则；若适配层与项目契约冲突，以项目契约为准并修复
适配层（参见 `AGENTS.md` §11 停止/升级条件）。

---

## 2. `.agents/` 项目级规则与技能

| 项 | 内容 |
|----|------|
| 规则 | `.agents/rules/code-organization.md`、`data-migration.md`、`package-manager.md` |
| 技能 | 当前无仓库锁定的第三方技能；`skills-lock.json` 保留空索引 |

`.agents/rules/` 是**项目级**代理规则（不是厂商配置）：`AGENTS.md` §2.4 直接引用
`code-organization.md` 的文件规模阈值，husky pre-commit 的 `scripts/check-file-size.sh`
实现同一门禁。HeroUI 技能已随前端迁移移除。

---

## 3. 嵌套 `apps/panel/AGENTS.md`

`apps/panel/AGENTS.md` 记录 shadcn/ui（Base UI）组件源码、CLI 与 Tailwind v4 的本地约定，
只作用于 `apps/panel` 子树。它不重复根契约规则；根契约始终优先。

---

## 4. 适配器符合性状态

按协议对 Agent Adapter 的符合性要求（能发现规则、能执行工作流、能执行或委派验证、
能暴露失败与阻塞、能满足完成定义），当前证据状态：

| 检查项 | `.agents/` 规则 |
|--------|:---------------:|
| 仓库规则可发现（指向 `AGENTS.md` / `docs/`） | ✅（`AGENTS.md` §2.4 引用） |
| 验证命令有明确出处 | ✅（`scripts/check-file-size.sh`） |
| 失败/阻塞可上浮 | 不适用（规则文件无执行体） |
| 执行记录（代表性任务） | 不适用 |

---

## 5. 维护约定

- 新增或调整编码代理：更新本文件对应小节 + `AGENTS.md` §9 同步表。
- 仓库结构调整后，运行
  `rg -n "cargo|bun|nix" .agents/rules apps/panel/AGENTS.md` 核对适配层命令与项目契约。
- 不要把厂商模型名、权限配置、skill 版本写入 `AGENTS.md` / `README.md` / `docs/architecture.md`。
