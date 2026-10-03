# Project Agents Guide

## 当前分层策略
- **orchestrator**（Kimi K3/DeepSeek V4 Pro）：规划 + 裁判 + 人机确认
- **executor**（DeepSeek V4 Flash）：纯执行
- **reviewer**（Kimi K3/DeepSeek V4 Pro）：执行后审查（可选）

## 使用约定
1. 默认与 orchestrator 对话。
2. 复杂任务先让它出计划，确认后再执行。
3. 执行中出问题优先让 orchestrator 判断。
4. 只有最终决策或高风险点才直接找我。

## 项目技术栈与规范

项目规则以根目录 [`AGENTS.md`](../AGENTS.md) 为准（构建、代码规范、提交规范、工作流、
完成定义与停止/升级条件），适配器侧的验证命令矩阵见
[`.opencode/rules/coding-standards.md`](rules/coding-standards.md)。

- 后端：Rust workspace（Axum + Tonic + Sea-ORM + tokio），13 个 crate
- 前端：Bun workspaces（依赖在仓库根目录安装，单一 `bun.lock`）——`apps/panel`（pp-web）、
  `apps/client`（pp-client-app，单壳双目标）、`packages/client-core`（@pp/client-core）
- 客户端壳：`apps/client/src-tauri`（独立 cargo 项目，不在根 workspace）

厂商特有的模型、权限与子代理配置属于适配器层，只写在本目录与
[`docs/adapters/README.md`](../docs/adapters/README.md)，不要写入项目契约。
规则优先级（项目契约 > `.agents/rules/` > 适配器配置）见 `docs/adapters/README.md` §1。