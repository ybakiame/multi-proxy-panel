# ADR-0004: 客户端移除 mihomo 核心支持（收敛为 sing-box 单核心）

- **Status:** Accepted
- **Date:** 2026-09-05
- **Deciders:** ProxyPanel Contributors
- **Scope:** `crates/pp-client`、`apps/desktop`（前端 + src-tauri）、`apps/android`（panel-core + Kotlin VPN 服务）

---

## 1. Context & Problem Statement

客户端（桌面 + Android）历史上并行支持 sing-box 与 mihomo 双核心：

- 双核心使订阅兼容门（ClashYaml 仅 mihomo）、覆写模板、本地规则层、规则集订阅、核心二进制管理、配置合成/注入全部按核心分叉，维护成本翻倍。
- Android 侧 mihomo 的实际定位已退化为「sing-box 无法解析订阅时的自动降级内核」（ADR-0002 阶段③的 fallback 维护模式决策），只修崩溃不加功能。
- `node_convert::mihomo_to_singbox` 双向转换能力早已存在，且 Profile 层本就「订阅只取节点、本地模板生成规则」，ClashYaml 订阅的 `proxy-groups`/`rules` 从不进入运行配置——ClashYaml→sing-box 转换在现有架构下无行为损失。

Hub/Agent 侧（`pp-hub`/`pp-agent`/`pp-config`/`pp-subscription`）的多核心支持**不在本次范围**，保持不变。

## 2. Decision

客户端收敛为 sing-box 单核心，具体决策：

1. **ClashYaml 订阅转 sing-box 放行**：拉取时经 `mihomo_to_singbox` 产出 sing-box 节点并缓存，取消「ClashYaml 仅 mihomo」硬门与 Android 自动降级；Clash 专属协议类型（ssr/snell 等）跳过并记 warning。
2. **存量数据自动归一化 + 清理**：`core_type: "mihomo"` 的旧 `client.json` 加载时归一化（mihomo 二进制引用重置为自动选择）并写回；`profiles.json` 中 mihomo 覆写模板加载时一次性剔除；`local_override.json` mihomo 桶与订阅缓存 mihomo 桶靠 serde 容忍静默丢弃；磁盘上 `cores/mihomo/` 已下载二进制在 `ClientCoreInventory` 构造时 best-effort 删除。
3. **旧版 Hub 订阅回退路径整体下线**：`fetch_singbox_config`/`fetch_clash_config` 与 state 的 deprecated Hub 分支删除，客户端只走本地订阅缓存路径。
4. **Android 精简 panel-core**：删除 `MihomoVpnService.kt` 与 `mihomocore/` Go wrapper，gomobile 只 bind libbox，mihomo 专用 GEO 资产（geoip.metadb/geosite.dat 等）与更新脚本删除。精简后 panel-core 仅剩 libbox 依赖锚定，未来可直接替换为官方 libbox AAR。

## 3. Consequences

### 正面

- 客户端配置合成、覆写、规则集、核心管理全部单分支，代码净删约 3000+ 行（pp-client）+ 1600+ 行（Android）。
- 订阅格式与核心解耦：三种订阅格式（ShareLinks/SingBoxJson/ClashYaml）统一由 sing-box 运行。
- UI 去掉核心类型维度（核心管理页、覆写模板、Dashboard 联动切核、格式门禁全部简化）。

### 负面 / 风险

- 仅 Clash 生态支持的协议（ssr/snell/hysteria1 等）节点将被跳过，用户会在订阅拉取日志看到 warning。
- `libs/panelcore.aar` 需在有 Android SDK/NDK 的机器上重跑 `apps/android/scripts/build-panel-core.sh` 重建，否则 AAR 内仍带 mihomocore 绑定（不再被引用，仅占体积）。
- `pp-common::CoreType` 枚举保留 `Mihomo` 变体（Hub 侧在用），客户端仅不再消费。

## 4. 关联

- 承接 ADR-0002 阶段③「sing-box 主核 + mihomo 冻结」路线的终点：mihomo 从 fallback 维护模式转为正式移除。
