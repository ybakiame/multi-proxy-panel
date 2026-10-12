# ADR-0015: Lanpath 客户端独立仓库与代码所有权

- **Status:** Accepted
- **Date:** 2026-10-11
- **Deciders:** 维护者于 2026-10-11 明确授权执行；Windows CI 原生锁定工具链例外已确认
- **Scope:** ProxyPanel / Lanpath 仓库所有权与构建边界

## 2026-10-11 清理补记

维护者已确认迁移验证完成并授权完整清理原仓客户端。ProxyPanel 工作区已迁至
`$HOME/code-workspaces/multi-proxy-panel`，本次移除客户端专属源码、共享前端包、文档、
构建产物与 CI；保留 Hub、Agent、管理 Web、CLI、`pp-common` 和 `pp-core`。
服务端共享模块仅移除客户端专属错误、端口诊断和预启动钩子，保留 Agent 的父子进程绑定、
PID 校验与启动收割。Nix、锁文件、Docker、钩子与 Rust 门禁按服务端范围收窄，Linux CI 检查使用 Nix；服务端 Release 保留既有双架构构建和部署兼容性。

客户端专属 ADR、计划与验证材料由 Lanpath 文档和原仓 Git 历史保留；本仓当前文档只维护
服务端契约及产品边界。以下正文记录拆仓时的原始决策，其中“尚未授权删除”已被本补记
的明确授权取代。本次清理不迁移或删除用户数据、不修改协议子模块，也不触发正式发布。

## Context

客户端已确定独立品牌岚径 / Lanpath，用户要求正式规划拆仓，并希望首次提交已经具备
正式命名。现有客户端依赖根 Rust/Bun workspace、共享 pp-common/pp-core、Nix 与 CI，
仅搬 apps/client 无法独立开发。服务端 Agent 仍使用 pp-core，不能整份移走。

## Decision

建立独立 Lanpath 仓库，桌面/Android 继续同仓。先完成正式应用身份及品牌整理，
再在隔离目录构建和验证整理后的初始快照；保留源仓历史与来源/许可/贡献归属记录。
正式应用标识统一为 com.ybakiame.lanpath，产品名 Lanpath。
本地新仓位置为 `$HOME/code-workspaces/lanpath`；本轮不移动原 ProxyPanel 仓库。
Go 内部模块品牌化为 `lanpath-core` / `lanpathcore.aar`。
新仓独立引入 `.protocol/agent-development-protocol`，保留协议修订与采纳证据。

客户端专属 Rust、Go、Kotlin、React 和 UI 代码随客户端迁移。共享基础与核心运行能力
按实际使用提取为客户端自有模块，服务端保留自己的实现；暂不建立跨仓公共 SDK。
新仓不得依赖 ProxyPanel 的检出、路径、Git 依赖或构建配置。

两仓平级维护，不将 Lanpath 作为 ProxyPanel 的 submodule 或 subtree。
产品间通过订阅/API 协作，构建与版本独立。新仓验收完成后才单独清理原仓客户端代码。

## Consequences

客户端拥有独立依赖、Nix、CI、发布与版本管理，避免继续继承服务端 workspace 元数据。
提取后的基础代码存在有限的共同来源，但所有权清晰，不要求双仓同步整份实现。
新增公共库须由真实复用需求证明，另行决定接口与版本。

应用标识改变涉及 Android 新应用身份及桌面数据/安装关系，必须明确处理方式。
本地及 Linux/Android 构建使用新仓 Nix；维护者明确允许 Windows CI 使用锁定版本
的原生 MSVC/Rust/Bun。签名、更新地址过渡与 Windows 真机仍需验收。
本决策不授权迁移用户数据、删除旧客户端或正式发布。
