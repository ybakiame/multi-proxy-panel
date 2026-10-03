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
| [ADR-0002](0002-client-rule-management.md) | Accepted | 2026-08-26 | 客户端规则管理交互重设计（本地 Override 层 + 规则卡片 + 规则集订阅） |
| [ADR-0003](0003-desktop-mobile-split.md) | Superseded by ADR-0007 | 2026-09-05 | 客户端 Desktop / Mobile 双应用分离架构（历史） |
| [ADR-0004](0004-client-singbox-only.md) | Accepted | 2026-09-05 | 客户端移除 mihomo 核心支持（收敛为 sing-box 单核心） |
| [ADR-0005](0005-client-config-slices.md) | Accepted | 2026-09-11 | 客户端可视化配置切片（Config Slices）与配置管理页面 |
| [ADR-0006](0006-client-traffic-stats.md) | Accepted | 2026-10-02 | 客户端流量统计（Clash API WebSocket 采集 + 本地 SQLite 聚合） |
| [ADR-0007](0007-client-single-shell-merge.md) | Accepted | 2026-10-03 | 客户端合并为单壳双目标应用（apps/client） |

> 关联计划与评估材料：`docs/plans/2026-10-03-client-merge-evaluation.md`（ADR-0007 的评估依据）、
> `docs/research/client-audit-2026-08.md`、`docs/research/panel-comparison.md`。

## 元数据修正记录

协议采纳（Pass 2）期间只修正了确证的元数据漂移，未改写任何决策正文：

- **ADR-0002**：`Status` 由 `Proposed` 改为 `Accepted`。该 ADR 的 §8.2 已记录 v1 实现完成
  （“前端（apps/desktop / apps/android）— ✅ v1 已完成”），实现证据与 `Proposed` 不符。
- **ADR-0006**：`Date` 由 `2026-12-19` 改为 `2026-10-02`。原日期晚于 ADR-0007（2026-10-03）
  且晚于仓库状态；该文件首次提交日期为 2026-10-02（`git log --diff-filter=A`），
  与其余 ADR 的“决策日期≈首次提交日期”惯例一致。

`apps/desktop` / `apps/mobile` 等历史路径出现在 ADR-0003/0004/0005/0006 的 Scope 与正文中，
属于当时的真实记录，按本文约定**不回溯改写**；当前客户端结构以 ADR-0007 与
`docs/architecture.md` 为准。
