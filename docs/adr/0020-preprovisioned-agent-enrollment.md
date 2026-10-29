# ADR-0020: 节点统一采用 Hub 预生成接入

- **Status:** Accepted
- **Date:** 2026-10-29
- **Deciders:** ProxyPanel 维护者
- **Scope:** `pp-hub`、`pp-agent`、`pp-cli`、安装脚本、部署配置与文档
- **Related:** ADR-0018

## Context

早期 Agent 可在启动时生成 ID 和 Token，并在 Hub 开启 `auto_register_agents` 时自动创建节点。当前面板已支持先创建节点、再生成包含节点 UUID、Token 和固定版本的安装指令。保留第二种注册路径会增加配置与测试分支，也使生产部署容易误开未经审核的节点接入。

## Decision

节点接入只保留 Hub 预生成流程：

1. 管理员在 Hub 创建节点并取得安装指令。
2. 安装指令在目标 Agent 服务器写入预生成 UUID 与 Token。
3. Hub 仅接受数据库中已存在且 Token 匹配的节点，未知 UUID 一律拒绝。

删除 `auto_register_agents` 配置、对应 CLI 环境覆盖和自动建库分支。Agent 不再为自动注册生成随机 Token；缺少安装 Token 与本地持久化 Token 时明确失败。旧配置中的该键不再生效，可随升级手工删除，不需要数据库迁移。

## Consequences

- 已预生成的现有节点不受影响。
- 依赖自动注册的节点必须先在面板创建，再使用新安装指令接入。
- 节点身份与接入授权统一由 Hub 管理，减少误配置和未审核接入。
- 安装命令仍包含长期 Token，需通过 systemd env 和文件权限保护；一次性短期接入凭证如需引入，应另行设计注册协议。
