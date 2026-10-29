# 架构决策记录（ADR）索引与约定

本目录保存 ProxyPanel 的架构决策记录（Architecture Decision Records）。
ADR 记录**为什么**做出某个重要决策，以及当时的权衡；它是历史记录，不随实现变更而改写。

> 位置约定：ADR 保留在 `docs/adr/`（不迁移到其他目录）。编号连续、文件名
> `NNNN-<kebab-case-title>.md`。

## 状态约定

每条 ADR 的 `Status` 必须是下列之一：

| 状态 | 含义 |
|------|------|
| `Proposed` | 已提出、尚未确认实施 |
| `Accepted` | 已确认并生效（实现可能已落地或进行中） |
| `Superseded by ADR-NNNN` | 已被后续 ADR 取代；保留原文，补充取代说明 |
| `Deprecated` | 不再推荐，但尚无替代方案 |
| `Rejected` | 讨论后未采纳 |

补充规则：

- 改变既有决策时**新增**一条 ADR 并标注 `Supersedes: ADR-NNNN`，同时把被取代 ADR 的状态
  改为 `Superseded by ADR-NNNN`；不要重写既有 ADR 的正文。
- 已实施完毕的历史背景补充（如某功能后续被移除）可在正文顶部加带日期的“补记”，
  但不得删除原有决策内容。
- 元数据（Status / Date / 链接）可以修正；决策正文不可静默改写。

## 必备字段

每条 ADR 至少包含：

```markdown
# ADR-NNNN: <标题>

- **Status:** Accepted | Proposed | Superseded by ADR-NNNN | Deprecated | Rejected
- **Date:** YYYY-MM-DD
- **Deciders:** <角色/团队>
- **Scope:** <受影响的 crate / app / 文档路径>
- **Supersedes:** ADR-NNNN（可选）
- **Related:** <相关 ADR、计划、研究文档>（可选）

## Context
## Decision
## Consequences
## Alternatives considered（可选）
```

## 索引

| ADR | 状态 | 日期 | 主题 |
|-----|------|------|------|
| [ADR-0001](0001-neutral-fields-for-multi-kernel-protocols.md) | Accepted | 2026-05-30 | 多核心协议配置的中性字段设计 |
| [ADR-0015](0015-lanpath-independent-repository.md) | Accepted | 2026-10-11 | Lanpath 独立仓库与代码所有权 |
| [ADR-0016](0016-central-log-persistence.md) | Accepted | 2026-10-13 | Hub 日志持久化、共享脱敏与集中查询 |
| [ADR-0017](0017-shadcn-base-ui-panel.md) | Accepted | 2026-10-25 | 管理面板采用 shadcn/ui 与 Base UI |
| [ADR-0018](0018-hub-self-update-maintenance.md) | Accepted | 2026-10-25 | Hub 受控自更新与部署授权模型 |
| [ADR-0019](0019-devenv-migration-and-rc-docker-skip.md) | Accepted | 2026-10-25 | 开发环境迁移至 devenv 与 RC 标签跳过 Docker 发布 |
| [ADR-0020](0020-preprovisioned-agent-enrollment.md) | Accepted | 2026-10-29 | 节点统一采用 Hub 预生成接入 |
| [ADR-0021](0021-native-devenv-cli.md) | Accepted | 2026-10-29 | 开发环境全面采用原生 devenv CLI |

> 客户端专属 ADR-0002–0014 随拆仓从当前工作区移除；决策原文和元数据修正记录
> 保留在 Git 历史及 Lanpath 文档中，不再作为 ProxyPanel 的当前开发契约。
> 本仓编号不重用，后续决策从 ADR-0020 继续。
