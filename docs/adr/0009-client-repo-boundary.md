# ADR-0009: 客户端产品定位与仓库边界（单仓过渡 + 拆分触发条件）

- **Status:** Accepted
- **Date:** 2026-10-06
- **Deciders:** ProxyPanel Contributors
- **Scope:** `apps/client`、`packages/client-core`、`crates/pp-client*` / `pp-mitm` / `pp-script`、`.github/workflows/ci.yml`、订阅契约
- **Related:** ADR-0007（单壳双目标）、ADR-0008（Windows 打包与分发）

---

## 1. Context

客户端（`apps/client`）与服务端（Hub/Agent/CLI）同处一个混合仓库。2026-10 的产品方向
讨论确认：**客户端是面向终端用户的独立产品**（自有版本号、发布节奏、分发渠道与用户群），
与服务端（运维组件）定位不同。

同仓维护的成本已经显现：

- **CI 互相拖累**：服务端改动触发客户端 Tauri/Android 链路检查；客户端打包问题
  （2026-10-05 release 失败：Docker stub 层漏 `pp-client-tauri`、Windows runner 缺 protoc）
  反过来阻塞服务端发版；
- **发布节奏被迫同步**：客户端 0.1.0 与服务端 0.4.5 版本号已分叉，却共享 tag 命名空间
  与同一条 release 流水线；
- 产品叙事混乱：面板用户与客户端用户是两群人。

同时实测的耦合面（2026-10-06，依据各 crate `Cargo.toml` 与 `cargo tree`）比直觉**薄**：

| 耦合点 | 内容 | 性质 |
|---|---|---|
| `pp-common` | 错误类型 / 基础类型 | 两侧共享，小而稳定 |
| `pp-core` | 核心进程管理（spawn/reload/版本探测） | Agent 与桌面客户端共享（如 `-D` 相对路径修复双侧受益） |
| 订阅契约 | `/sub/{token}` 公开 HTTP 端点 + 分享链接格式 | 唯一线上协议耦合 |

> **2026-10-06 清理记录**：实测时 pp-core 还拖着两个服务端 crate——`pp-config`
> 为死依赖（源码零引用，已删），`pp-proto` 仅被 `core_api`（在线用户查询 /
> mihomo 流量端点约定）使用且真实消费者只有 pp-agent（已随 `core_api` 下沉至
> pp-agent）。清理后 pp-core 仅依赖 pp-common，下述边界断言方可在 CI 成立。

客户端 crate（`pp-client` / `pp-client-tauri` / `pp-mitm` / `pp-script`）不依赖任何
服务端专属 crate（`pp-db` / `pp-proto` / `pp-config` / `pp-subscription`），并以 CI 断言
固化（D1）。

## 2. Decision

**维持单仓过渡，正式化边界，设定拆分触发条件。** 拆分是"何时"而非"是否"的问题，
但不在本轮执行。

### D1 依赖边界纪律（CI 强制）

客户端链路 crate 禁止依赖服务端专属 crate（`pp-db` / `pp-proto` / `pp-config` /
`pp-subscription`）。CI `rust` job 增加 `cargo tree` 断言，防止边界腐化。

### D2 CI 路径分片

CI 各 job 按变更路径条件执行（`dorny/paths-filter`）：

- `rust`：仅 Rust 相关路径（crates/、proto/、Cargo.*、工具链、CI 自身）变更时运行；
- `client-shells`：仅客户端壳及其 Rust 依赖链（`apps/client/src-tauri` + pp-client /
  pp-client-tauri / pp-mitm / pp-script / pp-common / pp-core）变更时运行；
- `web`：拆为 panel / client-core / client-app 三个条件化 verify 步骤，按
  `apps/panel`、`packages/client-core`、`apps/client`（排除 src-tauri）分别触发。

分片语义为**跳过无关门禁**而非降低覆盖：共享 crate（pp-common / pp-core）与根
manifest 变更仍触发全部相关门禁。跳过的 job 在 GitHub 必需检查中按成功计。

### D3 订阅契约演进方向

客户端与服务端之间唯一的线上契约（`/sub/{token}`）按"可跨仓演进"标准约束：

- 服务端响应保持**向后兼容**（只增不改字段语义）；
- 客户端解析保持**宽松**（忽略未知字段）；
- 后续如引入破坏性变更，以查询参数/响应头做版本协商，届时另立 ADR。

本轮不改代码，仅记录约束方向。

### D4 拆分触发条件

以下任一成立时启动拆分为独立仓库（另立 ADR 执行）：

1. 客户端需要独立发布节奏（如周更 vs 服务端月更）且 tag 冲突实际发生；
2. 客户端代码量 / 贡献者群体显著超过服务端；
3. 客户端需要与服务端不同的开源许可策略（`pp-mitm` / `pp-script` 为纯客户端 crate，
   拆出后可独立定许可）；
4. 产品形态最终确定且确认两侧独立演进。

**未来拆分形态（预案，非本轮实施）**：`proxypanel-client` 仓库带走 `apps/client`、
`packages/client-core`、`crates/pp-client` / `pp-client-tauri` / `pp-mitm` / `pp-script`；
`pp-common` / `pp-core` 留主仓，客户端仓库以 git 依赖 + 锁定 rev 消费（两者小且稳定，
漂移风险可控）；订阅契约加 golden 样例契约测试（主仓产出，客户端 CI 校验）。

**共享包归属的方向依据**：所有权跟随主消费者与变更驱动方——pp-core 的主消费者是
Agent（节点核心管理是其日常职责），pp-common 是全仓地基，故留主仓而非随客户端迁出
（避免服务端倒挂依赖客户端仓库，也避免客户端未来独立许可策略牵连服务端）。第三方
客户端对接 panel 走 `/sub/{token}` 订阅契约（D3）而非 Rust crate；pp-common/pp-core
留主仓后，Rust 技术栈的对接方（含未来 client 仓）经 git 依赖引用即可。

## 3. Consequences

**正面**：

- CI 无关门禁不再互相拖累（D2），服务端与客户端改动各自的反馈环变快；
- 依赖边界有 CI 断言兜底（D1），未来拆分时耦合面不会暗中扩大；
- 拆分决策从"时机争论"变为"触发条件核对"，降低未来决策成本。

**代价**：

- paths-filter 规则需要与仓库结构同步维护（新增 crate / 移动目录时更新过滤条件）；
- 跳过门禁意味着合并引入的跨侧破坏要由更粗的触发条件兜底——通过把共享 crate 与
  根 manifest 纳入两侧触发条件覆盖；
- 单仓的固有成本（发布节奏、叙事）在触发条件满足前仍然存在。

## 4. Alternatives considered

- **立即拆仓**：耦合面虽薄，但 pp-common/pp-core 的拆分处理（发版或 vendor）与双仓
  协调成本在产品形态未完全落地前属于过早支付；拆分预案已记录（D4），触发条件满足时
  执行成本可控，拒绝。
- **永不拆 + 不约束边界**：CI 拖累与边界腐化风险持续存在，与"独立产品"定位矛盾，拒绝。
- **客户端发独立 crate 仓库（ crates.io 发布 pp-common/pp-core）**：为拆仓预热的收益
  不抵 crates.io 发版维护负担，拒绝。
