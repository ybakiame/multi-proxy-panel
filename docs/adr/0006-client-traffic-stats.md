# ADR-0006: 客户端流量统计（Clash API WebSocket 采集 + 本地 SQLite 聚合）

- **Status:** Accepted
- **Date:** 2026-12-19
- **Deciders:** ProxyPanel Contributors
- **Scope:** 客户端（`pp-client` / `pp-client-tauri` / `packages/client-core` / `apps/desktop` / `apps/mobile`）

---

## 1. Context & Problem Statement

用户需要知道「本次使用中我的流量命中情况」以针对性调整分流规则：每次连接的 **域名/IP + 命中规则 + 使用的节点（出站）**，以及**今日流量**汇总（总量 / 已代理量）。

关键事实：

- 命中规则与出站链信息**只有核心路由层知道**，客户端侧（TUN/系统代理层）无法获得，因此数据源只能是核心的 Clash API `/connections`。
- Clash API 双端恒启用（ADR-0005 P1 必选切片；mobile Go 引擎带 `with_clash_api` 编译 tag），`pp-client` 已有 2s HTTP 轮询的连接 tracker（内存环形缓冲 500 条，重启即丢）。
- sing-box `/connections` 支持 WebSocket upgrade（v1.12.9 起，全量快照按 `?interval=<ms>` 周期推送，payload 与 HTTP 响应同构），桌面子进程与 Android 内嵌引擎（panel-core，sing-box v1.15.0-alpha.5）行为一致。

## 2. Decision

### D1 采集：WebSocket 推送优先，HTTP 轮询仅作最后兜底

tracker（`pp-client/src/connections/tracker.rs` + `ws.rs`）启动后：

1. 尝试 `ws://127.0.0.1:<port>/connections?interval=1000`（secret 经 `token` query 参数 + Bearer 头双通道传递）；
2. 握手返回普通 HTTP 响应（未 upgrade）→ 判定服务端不支持，**本会话永久回退** 2s HTTP 轮询；
3. 瞬时失败（拒绝/超时/会话中断）→ 2s 退避后重连，始终保持 WS 优先。

快照内容（全量连接列表）与 HTTP 轮询完全同构，`parse_connections_response` 解析代码两路复用。

### D2 记账：快照差分增量入账，而非关闭时结算

每轮快照差分产出 `TrackerBatch`：

- 存活连接：`upload/download` 对上一快照做**饱和减法**（计数器回退记 0，容忍核心重启复用连接 ID）；
- 新连接：`start >= tracker 启动时间` 才全量入账并计 `conn_count`，早前存活连接（App 重启但核心未重启的场景）只建基线，**避免重复计数**；
- 快照获取失败时**保留** `last_seen`（不再清空）：恢复后的差分能补上故障窗口内的字节，窗口内消失的连接按最后已知计数记为关闭。

增量入账使长连接（如下载）的今日流量随快照渐进可见，而非关闭时才一次性出现。代价：连接关闭瞬间与上一快照之间 ≤1s 的尾部流量不计入聚合（明细记录同理，取最后已知计数）。

### D3 存储：客户端本地 SQLite（`stats.db`），不引入 pp-db

- 位置 `<data_dir>/stats.db`，sqlx 直连（sqlite + WAL + busy_timeout），池固定单连接（写入为秒级小批量，串行足够且规避 `SQLITE_BUSY`）；schema 用 `PRAGMA user_version` 轻量版本管理。**不引入 pp-db / sea-orm-migration**（Hub 侧体系对客户端过重）。
- 两级表：
  - `daily_stats` 日聚合：`(date, target, destination_ip, rule, rule_payload, outbound)` 六元主键，`StatDelta` upsert 累加；保留 90 天；
  - `conn_records` 关闭连接明细：保留 7 天；打开时统一清理过期数据。
- `target` = 请求域名（有 sniff/请求域名时）否则目的 IP——聚合维度即「域名+IP」。
- 「已代理流量」口径：`LOWER(outbound) NOT IN ('direct','block','reject')`（`DIRECT_OUTBOUND_TAGS`，SQL 字面量需与其保持一致）。

### D4 架构落点：沿用双端共享分层

| 层 | 位置 | 内容 |
|---|---|---|
| 采集 + 存储 | `pp-client`（`connections` / `stats`） | 双端一份 |
| 命令 | `pp-client-tauri`（`commands/stats.rs`） | `stats_today` / `stats_daily` / `stats_records` / `stats_clear`；`AppState` 懒打开 `StatsStore`（打开失败降级为统计不可用、tracker 仅内存态，不阻断代理功能），`start_proxy` 注入 `ClientState` |
| 前端 API | `packages/client-core`（`api/stats.ts` + query keys） | 双端 UI 复用 |
| UI | `apps/desktop` / `apps/mobile` | 首页「今日流量」卡片（主指标 = 已代理流量，点击进入详情）+ `/stats` 详情页（聚合/明细双视图，服务端排序 + 模糊搜索） |

## 3. Consequences

- 统计精度定位为「规则命中分析」而非精确计费：WS 快照间隔 1s，短于间隔的极速生灭连接可能被漏记；总量可与 `/traffic` 全局计数器对账（未实现，留作后续）。
- 统计是本地数据，核心未运行也可查询历史（卡片与详情页不依赖核心存活）；采集只在核心运行 + Clash API 开启时进行。
- `ConnectionView` 新增 `target` / `destination_ip` / `outbound` 字段（`outbound` = chains[0] 叶子出站 tag），前端类型同步更新。
