# ADR-0005: 客户端可视化配置切片（Config Slices）与配置管理页面

- **Status:** Accepted
- **Date:** 2026-09-11
- **Deciders:** ProxyPanel Contributors
- **Scope:** 客户端（`pp-client` / `pp-client-tauri` / `packages/client-core` / `apps/mobile` 优先，`apps/desktop` 后置复用核心逻辑）

---

## 1. Context & Problem Statement

### 1.1 现状：5 层合成管线

客户端（ADR-0004 后为 sing-box 单核心）的运行配置由 `crates/pp-client/src/state/mod.rs` 的 `ClientState::start()` 编排，自上而下经 5 层合成：

| 层 | 位置 | 职责 |
|---|---|---|
| **① profile 层** | `profile/mod.rs`、`profile/overrides.rs` | `singbox_template` 本地模板 + YAML/JS 覆写（RFC 7386 深合并 + QuickJS 沙箱）；remote YAML → local YAML → remote JS → local JS。**全能力逃生舱口** |
| **② compose 层** | `core_config/compose.rs` | inbounds 整体替换（mixed / tun）、MITM 链注入（`compose_singbox_config`） |
| **③ local_override 层** | `local_override/schema.rs`、`local_override/singbox.rs`（ADR-0002） | 规则卡片 / 规则集 / 场景模板；`state::inject_local_override_warn_only` 在 compose 之后、panel features 之前注入 |
| **④ panel_features 层** | `core_config/singbox.rs` | 设置页强制注入（TUN、Clash API、出站模式、Android 强制 DNS 注入 `inject_android_dns`），**优先级最高** |
| **⑤ 启动** | `state/mod.rs` | desktop 落盘 spawn sing-box；Android JSON 字符串 → Kotlin `VpnPlugin` → libbox |

`pp-client` 与 Hub 侧 `pp-config` 完全独立：`pp-config` 负责 Hub ↔ Agent 的服务端节点配置，本 ADR 不涉及。

### 1.2 能力缺口

除 ADR-0002 已结构化的「规则 / 规则集」外，sing-box 顶层配置项的其余部分对普通用户仍是黑盒：

1. **DNS 无结构化 UI**：`dns.servers` / `dns.rules` / `dns.final` / `strategy` 只能经 YAML/JS 覆写编辑；移动端 WebView 下编辑 YAML/JS 体验极差，等同于不可用。
2. **自定义出站无 UI**：用户无法可视化新增一个 vless / vmess / shadowsocks / trojan / hysteria2 出站。`RuleAction::Outbound { tag }` 只能引用订阅节点或模板已有的 tag，无法引用用户新建的出站——规则与出站能力没有闭环。
3. **experimental 无 UI**：`cache_file` 等常用项只能写 YAML。
4. **入站无 UI**：移动端恒 TUN，普通用户无需暴露；仅混合端口等少数安全字段可能值得开放（列为 P2）。

### 1.3 既有隐患：预览与真实运行配置不一致

`crates/pp-client-tauri/src/commands/preview.rs` 的 `preview_core_config_impl` 只执行 `compose_singbox_config` + `apply_panel_features`，**未注入 local_override 层**（`state::start()` 中有 `inject_local_override_warn_only`，preview 没有）。结果是用户在预览弹窗看到的配置与核心实际运行配置不一致，规则调试时产生误导。本 ADR 一并修复该隐患。

### 1.4 差异化动机

参考项目 GUI.for.SingBox 验证了「顶层配置项分区可视化」的价值，但它是桌面形态且面向全量字段。移动端目前没有同类产品做「高价值字段精选 + 触屏特化」的配置管理——这是 ProxyPanel 移动端的差异化机会。

### 1.5 范围边界

- 仅客户端本地配置可视化，**不涉及 Hub / Agent / pp-config / 订阅生成**。
- 承接 ADR-0002 的「本地 Override 层」与 ADR-0003 的 desktop / mobile 分离架构；核心逻辑置于双端共享的 `pp-client` 与 `packages/client-core`。
- 与 ADR-0004 一致：单核心 sing-box，不引入 mihomo 分支。

---

## 2. Decision

### 2.1 配置切片（Config Slice）概念

引入**配置切片**：将 sing-box 顶层配置项拆为若干独立切片，每个切片 =

- 一个**独立结构化 schema**（只暴露精选字段）；
- 一个**内容驱动的注入规则**（有内容即生效，无切片级 `enabled` 总开关；见文末 P4 补记）；
- 一个**钻取式移动端子页面**（表单化编辑，全量 patch 保存）。

切片层与 local_override 层在语义上同属「本地配置层」（用户本地结构化配置，区别于订阅与远端模板），panel_features（第 ④ 层）仍是最高优先级。

### 2.2 切片清单与分期

| 优先级 | 切片 | 内容 | 状态 |
|---|---|---|---|
| **P0** | **DNS 切片** | `servers` / `rules` / `final` / `strategy` 表单化 | 首版 |
| **P0** | **自定义出站切片** | 可视化增删 vless / vmess / ss / trojan / hysteria2 等出站，供规则 `Outbound{tag}` 引用 | 首版 |
| **P0** | **规则页搬迁** | 规则卡片 / 规则集 / 场景模板整体迁入新「配置」Tab（原样搬迁，不重写逻辑） | 首版 |
| **P1** | **Experimental 切片** | `cache_file` 等；`clash_api` 已改为**必选切片**迁至配置页（恒启用、无切片级开关、仅调参数，见文末 P1 补记） | 已上线（P2） |
| **P2（或不做）** | **入站切片** | 移动端恒 TUN，仅可能暴露混合端口等少数安全字段 | 视需求 |
| **P2（或不做）** | **日志 / NTP 等** | 低价值长尾项 | 视需求 |

### 2.3 已确认决策点（D1–D6）

| 编号 | 决策点 | 结论 | 理由 |
|---|---|---|---|
| **D1** | Android DNS 冲突 | **方案 C**：DNS 切片在 Android 上提供「接管 / 跟随系统」显式开关，**默认跟随系统**（维持 `inject_android_dns` 强制注入）；用户选择接管时跳过强制注入，用户全权负责，UI 在接管模式下给出风险提示 | 既不破坏 Android 开箱可用（VpnService 接管后系统 resolver 不可用），又给高级用户接管入口 |
| **D2** | 与 YAML/JS 覆写的关系 | **方案 B**：可视化切片定位为「入门层」，YAML/JS 覆写仍是高级逃生舱口；两者并存，字段冲突时 **YAML/JS 覆写赢** | 不破坏存量高级用户；实现见 §3.2 |
| **D3** | 桌面端 | **方案 B**：核心逻辑（schema / 合成 / 校验）写在 `pp-client` 与 `client-core` 天然双端共享；UI 层 `apps/mobile` 先行，`apps/desktop` 后置复用 | 避免双份实现，复用 ADR-0003 的共享载体 |
| **D4** | 存储 | **方案 B**：新文件 `data_dir/config_slices.json`，与 `local_override.json` 并存；schema 内带 `version` 字段便于迁移；serde 全字段 `#[serde(default)]` 向后兼容 | 与现有 `local_override.json` / `profiles.json` 存储风格一致，互不干扰 |
| **D5** | 校验 | **方案 A**：前端表单即时校验 + Rust 落盘前 schema 校验；**不引入**启动前 sing-box check（Android libbox 无独立 check 命令） | 靠表单强校验与字段白名单控制风险 |
| **D6** | 首版范围 | **P0 仅 DNS 切片 + 自定义出站切片 + 规则页搬迁**；Experimental P1、入站 P2 | 控制爆炸半径 |

---

## 3. Detailed Design

### 3.1 `config_slices.json` schema 草案

> **已被 P4 补记取代**：下列草案中的切片级 `enabled` 总开关（`ConfigSlices` / `DnsSlice` / `OutboundsSlice` 等）已于 P4 移除，注入改为「有内容即生效」，`config_slices.json` 版本 1→2；下方代码块仅作历史草案保留，以文末 P4 补记为准。

新增 `crates/pp-client/src/config_slices/`，核心类型示意：

```rust
/// 配置切片总容器。存储位置：`data_dir/config_slices.json`。
///
/// 版本与兼容：`version` 预留迁移通道；所有字段 `#[serde(default)]`，
/// 旧版客户端无此文件时返回 `ConfigSlices::default()`（各切片 enabled=false）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigSlices {
    /// schema 版本号（便于将来迁移，当前 = 1）。
    #[serde(default = "default_slice_version")]
    pub version: u32,
    #[serde(default)]
    pub dns: DnsSlice,
    #[serde(default)]
    pub outbounds: OutboundsSlice,
}

/// DNS 切片。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsSlice {
    /// 切片总开关；false 时完全不注入 DNS 切片。
    #[serde(default)]
    pub enabled: bool,
    /// 平台 DNS 模式（仅 Android 有分歧，桌面恒按 Takeover 处理）。
    #[serde(default)]
    pub mode: DnsMode,
    /// DNS 服务器列表（精选字段）。
    #[serde(default)]
    pub servers: Vec<DnsServer>,
    /// DNS 分流规则（精选字段）。
    #[serde(default)]
    pub rules: Vec<DnsRule>,
    /// 兜底 DNS server tag（对应 sing-box `dns.final`）。
    #[serde(default)]
    pub final_tag: String,
    /// 全局解析策略。
    #[serde(default)]
    pub strategy: DnsStrategy,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsMode {
    /// 跟随系统：Android 维持 `inject_android_dns` 强制注入，本切片 DNS 正文不生效。
    #[default]
    FollowSystem,
    /// 接管：跳过强制注入，本切片 DNS 正文生效，用户全权负责。
    Takeover,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsStrategy {
    #[default]
    PreferIpv4,
    PreferIpv6,
    Ipv4Only,
    Ipv6Only,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsServer {
    /// 唯一 tag（切片内唯一，用于 `dns.rules` 与 `dns.final` 引用）。
    pub tag: String,
    /// 服务器地址（IP 或域名）。
    pub server: String,
    /// 服务器类型：udp / tls / https / quic / h3 / local。
    pub server_type: DnsServerType,
    #[serde(default)]
    pub server_port: Option<u16>,
    /// 出站 tag（可选，用于代理 DNS；如 `proxy`）。
    #[serde(default)]
    pub detour: String,
    /// 单服务器策略覆盖（可选）。
    #[serde(default)]
    pub strategy: Option<DnsStrategy>,
    /// 域名解析器 server tag（可选，避免自解析环路）。
    #[serde(default)]
    pub domain_resolver: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsServerType {
    Udp,
    Tls,
    Https,
    Quic,
    H3,
    Local,
}

/// 单条 DNS 分流规则（渲染为 sing-box `dns.rules` 元素）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsRule {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub match_type: DnsMatchType,
    /// 匹配目标（domain / suffix / rule_set tag 等，按 match_type 语义变化）。
    pub target: String,
    /// 命中的 DNS server tag（对应 `action: "route"`）。
    pub server_tag: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsMatchType {
    Domain,
    DomainSuffix,
    DomainKeyword,
    RuleSet,
}

/// 自定义出站切片。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundsSlice {
    /// 切片总开关；false 时完全不注入自定义出站。
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub items: Vec<CustomOutbound>,
}

/// 单个自定义出站。`protocol` 为结构化枚举，渲染时翻译为 sing-box outbound JSON。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomOutbound {
    /// 唯一 ID（前端生成 UUID v4）。
    pub id: String,
    /// 显示名称（用于生成 tag 与 UI 展示）。
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 协议特定字段（结构化枚举，按协议 tag 序列化）。
    #[serde(flatten)]
    pub protocol: OutboundProtocol,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutboundProtocol {
    Vless(VlessOutbound),
    Vmess(VmessOutbound),
    Shadowsocks(ShadowsocksOutbound),
    Trojan(TrojanOutbound),
    Hysteria2(Hysteria2Outbound),
    // 后续按需扩展 anytls / tuic 等
}

/// 各协议结构体仅含「精选字段」，未暴露字段一律不进入 schema，靠 YAML/JS 覆写兜底。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VlessOutbound {
    pub server: String,
    pub server_port: u16,
    pub uuid: String,
    #[serde(default)]
    pub flow: String,
    #[serde(default)]
    pub tls: OutboundTls,
    #[serde(default)]
    pub transport: OutboundTransport,
}

// VmessOutbound / ShadowsocksOutbound / TrojanOutbound / Hysteria2Outbound 结构同理，
// 分别承载各自协议的高价值字段（uuid/alter_id/security、method/password、
// password、password/obfs 等）。

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundTls {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub server_name: String,
    #[serde(default)]
    pub insecure: bool,
    #[serde(default)]
    pub alpn: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboundTransport {
    /// tcp / ws / grpc / http / quic。
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub host: String,
}
```

**字段精选原则**：只暴露高价值字段；未暴露字段不进入 schema，靠 YAML/JS 覆写兜底（D2）。

**分组出站（P1 落地）**：`OutboundProtocol` 在协议节点之外新增 `Selector` / `UrlTest` 两种分组类型（serde 判别值 `selector` / `urltest`，注意 `urltest` 无下划线）。分组成员范围 = 订阅节点 tag + 其他切片节点出站 tag + 内置 `direct`；**v1 禁止嵌套分组**（成员不得引用另一个分组）。Rust 落盘校验覆盖：成员非空、禁自引用、selector `default` 必须 ∈ 成员、`slice-` 前缀成员必须指向存在的启用切片出站（其余无法静态解析的 tag 交由 sing-box 运行期校验）。

**tag 冲突**：自定义出站 tag 强制前缀 `slice-`（由 `name` 生成 slug），注入时与订阅节点 / 模板出站做冲突检测，必要时追加稳定后缀。规则卡片的 `Outbound{tag}` 下拉只列出「订阅节点 + 模板出站 + 切片出站」的并集。

### 3.2 合成顺序与 D2 自洽

**核心问题**：后应用的层覆盖先应用的层；D2 要求 YAML/JS 覆写赢。因此切片必须在 profile 覆写**之前**应用。

为满足 D2，把管线重新表述为 ⓪→①→②→③→④→⑤：

```
⓪ 切片层（本地配置层·构建期注入）
   singbox_template(nodes) → apply_config_slices()   ← config_slices.json（DNS / 自定义出站）
        ↓
① Profile 覆写层（高级逃生舱口）
   remote YAML → local YAML → remote JS → local JS（RFC 7386 深合并 / QuickJS 沙箱）
        ↓
② compose 层
   compose_singbox_config()：inbounds 整体替换（mixed / TUN）、MITM chain 注入
        ↓
③ local_override 层（本地配置层·运行期注入）
   apply_local_override() + apply_custom_rule_sets()：规则卡片（全部 enabled 规则）/ 规则集
        ↓
④ panel_features 层（最高优先级）
   apply_panel_features()：TUN / Clash API / 出站模式 / inject_android_dns
        ↓
⑤ 启动
   desktop：落盘 spawn sing-box；Android：JSON → Kotlin VpnPlugin → libbox
```

**自洽说明**：

1. **切片层与 local_override 同属「本地配置层」**（语义分层），但**实际注入点不同**：切片在 ⓪（作为模板构建的输入），规则在 ③。这是因为 D2 要求 YAML/JS 赢，而管线是「后应用覆盖先应用」——把切片放在 profile 之前，YAML/JS 自然覆盖切片，**无需字段级 diff，实现最简单，也符合「覆写是逃生舱」的心智模型**。
2. **规则引用切片出站 tag 可解析**：出站实体在 ⓪ 注入，规则在 ③ 解析 `Outbound{tag}`，顺序上 ⓪ 早于 ③，引用成立。
3. **Android DNS（D1）**：⓪ 注入切片 DNS 正文；④ 的 `inject_android_dns` 在 FollowSystem 模式下整体覆盖 `dns`（切片正文不生效），Takeover 模式下跳过强制注入（切片正文生效）。
4. **边界**：若 ① 的 YAML/JS 删除了切片出站，而 ③ 规则仍引用该 tag，sing-box 启动会报错——这是「覆写赢」的预期代价，由高级用户负责；规则保存时的引用校验无法感知 YAML/JS（见 §3.7 风险）。

`state::start()` 的改动：在 `build_core_config_v2` 内部（模板构建后、YAML/JS 覆写前）调用 `apply_config_slices`；local_override 注入点维持现状（compose 之后、panel features 之前）。

### 3.3 UI 设计：新「配置」Tab

**Tab 更名**：`apps/mobile/src/components/TabBar.tsx` 的「规则」Tab（`/rules`）更名为「配置」，路由前缀迁移为 `/config`。

**路由迁移策略**：保留 `/rules/*` → `/config/*` 的 `<Navigate>` 重定向（HashRouter），或直接改路由；由实施阶段定。子路由：

```
/config                    # 入口页：EntryLinkCard 列表
/config/dns                # DNS 切片表单
/config/outbounds          # 自定义出站（节点 + selector/urltest 分组）列表 + 编辑
/config/network            # 网络（TUN）必选切片表单（混合端口；旧 /settings/network 重定向）
/config/clash-api          # Clash API 必选切片表单（API 端口与密钥；旧 /settings/clash-api 重定向）
/config/rules              # 规则卡片（原 /rules/custom 搬迁）
/config/rulesets           # 规则集管理（原 /rules/rulesets 搬迁）
/config/rulesets/market    # 规则集市场（原 /rules/rulesets/market 搬迁）
/config/experimental       # Experimental 切片（cache_file）表单
```

**入口页**：沿用现有 `EntryLinkCard`（`apps/mobile/src/components/EntryLinkCard.tsx`）列表，自上而下：总开关 / DNS / 自定义出站 / 网络（TUN）/ 规则 / 规则集 / Clash API。现有 `MasterSwitchCard` 原样保留；`TemplateSection`（场景模板）已随 P1 移除（见文末 P1 补记）。（**P4 修订**：切片级与规则总开关均已移除，「总开关」项与 `MasterSwitchCard` 不再存在，以文末 P4 补记为准。）

**子页**：沿用 `BackHeader`（`apps/mobile/src/components/BackHeader.tsx`）+ 底部 Sheet 表单（`Modal.Container placement="bottom"`，见 `RuleEditSheet.tsx` / `MobileSelectSheet.tsx`）+ `MobileSelectSheet`（枚举选择）。交互约定：

- **全量 patch 保存**：编辑态在内存，退出/保存时整体落盘（对齐现有 `localOverrideSave` 的全量 patch 模式）。
- **重启生效提示**：核心运行中保存成功后 toast 追加「重启代理后生效」（对齐现有 `toastRuleSaved`）。
- **触达区**：沿用移动端规范，可点元素 ≥44px；子页处理 `env(safe-area-inset-*)`（`PageShell`）。
- **DNS 接管风险提示**：`mode == takeover` 时在 DNS 子页顶部显示风险 Alert（「接管 DNS 后配置错误可能导致断网，请确保 `final` 服务器有效」）。

**桌面端（D3 后置）**：UI 层 `apps/desktop` 后续复用同一套 schema / 合成逻辑，Sheet 改为居中 Modal（对齐 ADR-0002 的桌面适配方式）。

### 3.4 preview 修复

`preview_core_config_impl`（`crates/pp-client-tauri/src/commands/preview.rs`）补齐切片层与 local_override 注入，使预览 = 真实运行配置：

- 在 `build_core_config_v2` 路径中接入 `apply_config_slices`（切片层）；
- 调用与 `state::start()` 相同的 local_override 注入逻辑（建议把 `inject_local_override_warn_only` 抽为 `pp-client` 的公共函数，`start()` 与 preview 共用，消除双真相源）；
- **例外注明**：preview 无法复现 MITM 链（`start()` 在运行时才拿到 `MitmChain` 监听地址），预览中的 inbounds / `pp-mitm` outbound 与运行态可能不同，UI 需标注该例外。

### 3.5 校验策略（D5）

- **前端表单即时校验**：必填、格式（IP / 端口 / UUID）、tag 唯一性、`dns.final` 必须指向已定义的 server tag、出站 `server` / `port` 合法。
- **Rust 落盘前 schema 校验**：`ConfigSlicesStore::save` 前做一次强校验（tag 唯一、枚举合法、引用完整），校验失败返回明确错误，不写盘。
- **不引入**启动前 sing-box check（Android libbox 无独立 check 命令）；靠表单强校验与字段白名单控制风险。

### 3.6 存储与迁移（D4）

- **存储路径**：`data_dir/config_slices.json`，与 `local_override.json` / `profiles.json` / `subscriptions.json` 同级。
- **默认值**：所有切片 `enabled = false`（opt-in），文件缺失或损坏时回退 `ConfigSlices::default()`，不阻塞核心启动。
- **版本兼容**：`version` 字段 + serde 全字段 `#[serde(default)]`；未来 schema 变更走版本化迁移（对齐 `ProfileStoreV2` / `LocalOverrideStore` 的 legacy 策略）。
- **写入时机**：前端全量 patch → `configSlicesSave` Tauri 命令 → 校验通过后写盘。

### 3.7 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| **用户 DNS 接管配错断网** | 中 | 高 | Android 默认 `follow_system`；接管模式 UI 风险 Alert；`final` server 必填校验；一键切回跟随系统 |
| **切片出站 tag 与订阅节点 / 模板 tag 冲突** | 中 | 中 | tag 强制 `slice-` 前缀 + 注入时冲突检测；规则下拉只展示合法并集 |
| **YAML/JS 覆写删除切片出站，规则引用悬空** | 低 | 中 | D2 既定行为（覆写赢），文档明确；规则保存时校验引用存在；启动失败时日志定位 |
| **schema 演进导致旧数据不兼容** | 低 | 中 | `version` 字段 + serde default；加载失败回退空配置不阻塞启动 |
| **Android DNS 双模式测试矩阵翻倍** | 中 | 中 | 双模式纳入回归用例；FollowSystem 与现状等价（风险最低路径） |
| **preview 与运行态仍有差异（MITM 链）** | 低 | 低 | 预览 UI 明确标注 MITM 例外 |

---

## 4. Consequences

### 正面

- **移动端差异化**：移动端首个「高价值字段精选 + 触屏特化」的配置管理，弥补 GUI.for.SingBox 等桌面形态的空缺。
- **入门用户可用**：DNS / 自定义出站无需编写 YAML/JS，降低门槛。
- **规则 / 出站闭环**：规则可直接引用可视化新建的出站 tag，不再受限于订阅节点。
- **消除预览隐患**：`preview_core_config` 与真实运行配置对齐（MITM 例外除外）。
- **双端共享**：核心逻辑位于 `pp-client` / `client-core`，桌面端可低成本后置复用。

### 负面 / 成本

- **双真相源心智成本**：切片与 YAML/JS 并存，用户需理解「覆写赢」的优先级；文档与 UI 需明确标注。
- **Android DNS 双模式测试矩阵**：FollowSystem / Takeover 两套路径需分别回归。
- **新增存储与 Tauri 命令**：`config_slices.json` + `configSlicesLoad/Save` 等命令，前端需新增 api / hooks / atoms。

### 后续工作

- **P1**：Experimental 切片（`cache_file` 等；`clash_api` 维持设置页）。
- **P2（或不做）**：入站切片（混合端口等）、日志 / NTP。
- **桌面端 UI**：`apps/desktop` 复用核心逻辑，Sheet 适配为居中 Modal。
- **文档同步**：`docs/architecture.md` 的客户端配置合成链路、`docs/api_reference.md` 的新 Tauri 命令。

---

## 5. Alternatives Considered

### D1（Android DNS 冲突）

| 方案 | 描述 | 否决理由 |
|---|---|---|
| A. 强制切片 DNS | Android 始终用切片 DNS，删除 `inject_android_dns` | 破坏开箱可用；VpnService 接管后系统 resolver 不可用，配错即断网 |
| B. 维持强制注入 | Android 完全忽略切片 DNS | 高级用户无法接管，能力缺口仍在 |
| **C. 显式开关（选定）** | 默认跟随系统，用户可选接管 | 兼顾开箱可用与高级接管 |

### D2（与 YAML/JS 覆写的关系）

| 方案 | 描述 | 否决理由 |
|---|---|---|
| A. 切片赢 | 切片在 profile 覆写之后应用，结构化配置优先 | 破坏存量高级用户 YAML/JS 覆写；违背「覆写是逃生舱」心智 |
| **B. YAML/JS 赢（选定）** | 切片作为模板输入在 profile 覆写前应用 | 实现最简单、无需字段级 diff、不破坏存量用户 |

---

## 6. 关联与相关文件

- 承接 **ADR-0002**（本地 Override 层 / 规则卡片 / 规则集 / 场景模板）：本 ADR 的「规则页搬迁」原样复用其能力，切片层与其同属「本地配置层」。
- 依赖 **ADR-0003**（Desktop / Mobile 分离）：核心逻辑共享、UI 移动端先行。
- 一致于 **ADR-0004**（sing-box 单核心）：不引入 mihomo 分支。

**相关文件**：

- `crates/pp-client/src/state/mod.rs` — `ClientState::start()` 合成管线与 local_override 注入点
- `crates/pp-client/src/profile/mod.rs`、`profile/overrides.rs` — profile 层模板与 YAML/JS 覆写
- `crates/pp-client/src/core_config/compose.rs` — compose 层
- `crates/pp-client/src/core_config/singbox.rs` — panel_features 注入（含 `inject_android_dns`）
- `crates/pp-client/src/local_override/schema.rs` — ADR-0002 本地 Override schema
- `crates/pp-client-tauri/src/commands/preview.rs` — 待修复的预览命令
- `crates/pp-client-tauri/src/commands/local_override/` — 现有本地 Override 命令实现
- `packages/client-core/src/api/localOverride.ts` — 现有前端 api 封装
- `apps/mobile/src/components/TabBar.tsx`、`EntryLinkCard.tsx`、`BackHeader.tsx`、`MobileSelectSheet.tsx` — UI 载体
- `apps/mobile/src/pages/Rules/index.tsx` — 待更名 / 搬迁的规则主页

---

*本 ADR 的 D1–D6 已与项目所有者确认；实施按 P0（DNS 切片 + 自定义出站切片 + 规则页搬迁）起步，控制爆炸半径。*

---

## 实施补记（2026-09-11）

P0（DNS 切片 + 自定义出站切片 + 规则页搬迁）已落地。以下为实施期确认的版本基线、已知限制与交互约定，**各项均已随 P0 提交落地**；本补记不改变 §2.3 的 D1–D6 决策，Status 仍为 Accepted。

### 渲染目标版本基线

- **sing-box >= 1.14.0，不做旧版本适配**。已核实切片渲染产出的全部字段（DNS `servers` / `rules` / `final`、各协议出站字段等）在 sing-box 1.14 均有效。

### 已知限制

- **规则「指定出站」下拉的选项来源**：订阅节点 / 模板出站选项来源于**运行中核心的 Clash API**；核心未运行时下拉仅列出切片出站。后续可新增静态节点列表命令改善。
- **DNS 规则 `match_type` 首版限定** `domain` / `domain_suffix` / `domain_keyword`；`rule_set` 待规则集引用闭环后（P1）解锁。
- **DNS schema 保留 per-server `strategy` 字段但不渲染**：sing-box 1.12+ 已无此字段，保留仅为 forward-compat。

### 交互约定

- **DNS / 出站页采用「内存草稿 + 显式保存」**，区别于规则页的即时保存。

### P1 补记（2026-09-11）

P1（必选切片 + 分组出站）与「场景模板移除」已落地。以下内容**修订** §2.2 / §3.1 / §3.3，其中**推翻** §2.2 原文「`clash_api` 维持设置页管理，不重复暴露」的表述；本补记不改变 §2.3 的 D1–D6 决策，Status 仍为 Accepted。

#### 场景模板移除与规则注入语义变更

- **功能删除**：场景模板（内置模板 + 用户自定义模板）已从 Rust（`local_override/template.rs`、Tauri 命令 `local_override_apply_template` / `revert_template`）与前端（mobile / desktop 的 `TemplateSection` / `FormSheet`、`client-core` 模板 api）整体移除。
- **兼容字段**：`LocalOverride.applied_templates` / `custom_templates` 保留为 serde 兼容字段（恒空），`load` 时一次性清理，写回恒为空数组。
- **注入语义变更**：local_override 层的规则注入条件由「`enabled` 且被已应用模板引用的规则」改为 **所有 `enabled` 规则全部注入**。
- **存量用户影响（行为变化需明示）**：升级前，未被任何已应用模板引用的启用规则**不会**注入运行配置；升级后，这些启用规则会**立即开始注入**，可能改变既有分流行为。用户应检查规则卡片列表，对不希望生效的规则显式关闭 `enabled`。
- 原「场景开关」需求由**规则卡片启用开关 + 分组出站**组合取代。

#### 必选切片（Mandatory Slice）

- **定义**：必选切片是切片模型的特例——**恒定启用、无切片级 `enabled` 关闭开关**，配置页仅提供参数调整。
- **网络（TUN）切片**：TUN 入站为代理基础能力（Android 由 `panel_features_tun_enabled` 恒注入），迁入配置页 `/config/network`，仅暴露本地混合端口（`mixed_port`）。旧 `/settings/network` 路径保留 `<Navigate>` 重定向。
- **Clash API 切片**：Clash API 为仪表盘与节点页数据源，迁入配置页 `/config/clash-api`，仅暴露 API 端口与密钥；原 `clash_api_enabled` 关闭开关移除，存量 `false` 在进入页面时静默纠正为 `true` 并落盘。旧 `/settings/clash-api` 路径保留重定向。

#### 分组出站（selector / urltest）

- **上线路径**：`OutboundProtocol` 新增 `Selector` / `UrlTest`（serde 判别值 `selector` / `urltest`）；Rust schema / 校验 / 渲染（`config_slices/`）→ `client-core` 类型 → 移动端出站页「分组 / 节点」两分区与成员多选 Sheet → 规则「指定出站」下拉自动纳入分组 tag。
- **v1 限制**：
  - **禁嵌套分组**：分组成员不得引用另一个分组。
  - 成员范围 = 订阅节点 tag + 切片节点出站 tag + 内置 `direct`。
  - Rust 校验：成员非空、禁自引用、selector `default` 必须 ∈ 成员、`slice-` 前缀成员必须指向存在的启用切片出站（其余无法静态解析的 tag 交由 sing-box 运行期校验）。
- §3.1 的「自定义出站切片」由单纯协议节点扩展为「协议节点 + 分组」。

### P2 补记（2026-09-11）

P2（静态节点列表命令 / DNS rule_set 闭环 / Experimental 切片上线 / 规则页 UX 变更）已落地。以下内容**修订** §2.2 / §3.3，并**解除** P0 / P1 补记中的两项已知限制；本补记不改变 §2.3 的 D1–D6 决策，Status 仍为 Accepted。

#### 静态节点列表命令（解除 P0「核心未运行时仅列切片出站」限制）

- **新 Tauri 命令** `subscription_node_tags(subscription_id)`：由 `pp-client` 的 `cached_node_tags` 从 `data_dir/subscription_cache/<id>.json` 读取节点 tag，**不依赖核心运行、不依赖网络**；缺失 / 损坏缓存返回空列表（与 `proxies_list` 的运行期 Clash API 数据源互补）。
- **tag 口径一致**：复用配置生成同一路径（`extract_nodes_singbox`）提取节点，故静态 tag 与运行期 Clash API proxy 名一致（含叶子类型过滤与重名 `-2` / `-3` 后缀）。
- **前端接入**：移动端规则「指定出站」下拉与分组成员候选 = **静态订阅节点 + 运行中模板分组 + 切片出站**的并集去重。
- **限制解除**：P0 补记「规则『指定出站』下拉的选项来源……核心未运行时下拉仅列出切片出站」的已知限制**已解除**——核心未运行时也能列出订阅节点。

#### DNS 规则 rule_set 匹配闭环（解除 P0「rule_set 待 P1 解锁」限制）

- **注入扫描扩展**：`apply_custom_rule_sets` 的引用采集范围由「route 规则卡」扩展为「route 规则卡 + 已渲染 config 的 `dns.rules[].rule_set` 引用」，两类引用合并去重（DNS 切片在 ⓪ 层先于 local_override 的 ③ 层，故此处读取的是已渲染 DNS 正文）。
- **双形态兼容**：`rule_set` 匹配字段接受**单字符串**与**字符串数组**两种形态，非法值忽略；同一 tag 被 route 与 DNS 同时引用只产出一条 local rule_set 条目（幂等）。
- **remote / bundled 天然可引用**：内置与远程规则集本就「enabled 即注入」，无需引用驱动扫描，故 DNS 规则可直接引用。
- **前端**：DNS 规则表单恢复 `rule_set` 匹配选项；规则集选项抽为共享数据源 `buildRuleSetOptions`，路由规则与 DNS 规则**同源**（口径 = custom 规则集全集 ∪ enabled 的内置引用）。
- **限制解除**：P0 补记「DNS 规则 `match_type` 首版限定 `domain` / `domain_suffix` / `domain_keyword`；`rule_set` 待规则集引用闭环后（P1）解锁」的已知限制**已解除**。

#### Experimental 切片上线

- **schema**：新增 `ExperimentalSlice { enabled, cache_file: CacheFileSlice }`，`CacheFileSlice { enabled, path, cache_id, store_fakeip }`；全字段 `#[serde(default)]`，`ConfigSlices` 增加 `experimental` 字段。
- **渲染**：`apply_config_slices` 将 `cache_file` **深合并**写入 `config.experimental.cache_file`，保留 `clash_api` 等同级键（`clash_api` 仍由 ④ panel_features 层拥有）；`cache_file.enabled=false` 显式写出以关闭缓存。
- **字段精选结论（按 sing-box 1.14 `option.CacheFileOptions`）**：
  - 仅暴露 `enabled` / `path` / `cache_id` / `store_fakeip`；
  - `store_timeout` **在 sing-box 1.14 不存在**，不引入；
  - `store_rdrc` / `rdrc_timeout` **已废弃**，不引入；
  - `store_dns`（1.14 新增）暂未暴露，留待后续按需纳入。
- **前端**：移动端 `/config/experimental` 页面（`cache_file` 表单）上线，`configSlices` api 补齐。

#### 规则页 UX 变更

- **无运行实例时出站模式空显修复**：`proxy_status` / `stop_proxy` 走 `idle_status_view` 归一化，核心未运行时返回归一化的出站模式而非空白。
- **首页启停改为 FAB**：原启停操作卡移除，改为悬浮按钮（`StartStopFab`）；Alert 独立条件渲染，不再依赖操作卡容器。

### P3 补记（2026-09-12）：FakeIP 模式 / IPv6 开关 / TUN 双栈化 / 内嵌面板

本补记记录 P3 的客户端网络能力变更（FakeIP、IPv6 开关、TUN 双栈化）与内嵌面板决策。以下内容**修订** §2.2 / §3.1 / §3.3 的字段与页面清单，不改变 §2.3 的 D1–D6 决策，Status 仍为 Accepted。

#### 内置 FakeIP 模式（opt-in，双路径）

- **动机**：DNS 查询立即返回虚拟 IP、连接时按域名由出站解析，规避本地 DNS 污染与远程 DoH 脆弱链路。对齐参考仓库 `qixuancao/sing-box-config-templates` 的 `android/fakeip.json`，但 FakeIP 覆盖**全部 A 查询**且**不依赖规则集**。
- **路径一·内置开关** `ClientConfig.dns_fakeip_enabled`（默认 `false`，`#[serde(default)]` 兼容旧 `client.json`）。开启时 ④ panel_features 层 `apply_fakeip_mode`（`core_config/fakeip.rs`）执行：向 `dns.servers` 追加 fakeip server（tag `fakeip`，`inet4_range` `198.18.0.0/15`）；向 `dns.rules` **头部**前插两条——`query_type:[HTTPS,SVCB]`（IPv6 关闭时并入 `AAAA`）→ `predefined` + `NOERROR` 丢弃、`query_type:[A]` → `route` 到 `fakeip`；深合并 `experimental.cache_file` 置 `enabled` + `store_fakeip`（保留 `clash_api` 等同级键）。不改 `dns.final` / `dns.strategy`（后者归 IPv6 开关）。实现**幂等**（按 tag 与精确 query-type 集合 + action 判定），且仅在已存在 `dns` 对象时注入。
- **路径二·切片 schema**：`DnsServerType::Fakeip`（`inet4_range` / `inet6_range`，空 `inet4_range` 渲染默认 `198.18.0.0/15`）、`DnsMatchType::QueryType`（逗号分隔 target → 渲染大写数组，按白名单校验）、`DnsRuleAction::{Route,Predefined,Reject}`（`predefined` 带 `rcode`，白名单 `NOERROR/FORMERR/SERVFAIL/NXDOMAIN/NOTIMP/REFUSED`）；旧数据 serde default 兼容，渲染抽到 `dns_render.rs`。接管模式下用户可用切片自建 FakeIP（或任意 DNS），与内置开关互不冲突。
- **前端**：client-core 类型 `inet4_range` / `inet6_range` / `action` / `rcode` 为**非 Option 必填 string**（serde default 恒在，恒可安全读）；DNS 页在 `follow_system` 模式新增 FakeIP 开关卡（接管模式隐藏），服务器表单支持 `fakeip`，规则表单支持 `query_type` 与三种动作。

#### IPv6 开关与 FakeIP / takeover 的相互作用

- **IPv6 开关**：`ClientConfig.ipv6_enabled` 默认 `false`，关闭时 ④ 层把 `dns.strategy` 覆写为 `ipv4_only`；须在 `inject_android_dns`（整段重写 `dns` 且 `strategy=prefer_ipv4`）**之后**执行，Android 上才能生效。
- **FakeIP 顺序**：FakeIP 注入在 IPv6 覆写之后最后执行，故 `AAAA` 是否随 `HTTPS/SVCB` 一并丢弃取决于 `ipv6_enabled`（关闭才丢 `AAAA`）。
- **接管豁免（ADR-0005 D1）**：`dns_mode == Takeover` 时，IPv6 `strategy` 覆写与 FakeIP 注入**整体跳过**——DNS 已在 ⓪ 层由用户切片完全接管，内置逻辑不得覆盖。

#### TUN 双栈化与 IPv6 开关的动机（AAAA 黑洞 / 节点无 v6 出栈）

- **问题**：TUN 入站此前仅配置 IPv4 地址（`172.19.0.1/30`），而 DNS 策略 `prefer_ipv4` 仍会对双栈域名返回 AAAA。App 优先尝试 IPv6 时 VPN 无 v6 路由，数据包进不了 TUN，导致有 AAAA 记录的域名直连失败/黑洞。
- **TUN 双栈化**（b328249）：改为官方双栈地址 `["172.19.0.1/30", "fdfe:dcba:9876::1/126"]`，该函数双端共用（Android 仅额外 `strict_route`），v6 流量进入 TUN 后按路由规则走代理/direct，不再泄漏或超时。
- **IPv6 开关**（d7aad04 / ba39242）：默认关闭并把 `dns.strategy` 设为 `ipv4_only`，从 DNS 层规避「节点无 v6 出栈时 AAAA 连接失败」；TUN 双栈与 `ipv4_only` 两者互补（前者兜住已产生的 v6 流量，后者从源头不再返回 AAAA）。

#### 内嵌面板替代原生代理 / 连接页

- **决策**（163a47c）：移动端新增 `/panel` 页，以 **iframe** 内嵌内核 Clash API `/ui` 路径提供的 zashboard 面板；移除原生 `/proxies` / `/connections` 页面，旧路径以 `<Navigate>` 重定向到 `/panel`（HashRouter 存量书签兼容）。
- **iframe 路线**：直接复用核心 `external_ui` 内置服务，不维护自有面板 UI；核心未运行 / Clash API 未就绪时展示空态提示。
- **约束**：受 Tauri Android 单 WebView 约束，无法以独立 WebView 控件承载面板，故采用同页 iframe 复用系统 WebView。

### P4 补记（2026-09-12）：CN 分流基线 / FakeIP 架构修正 / 开关移除 / 基线视图

本补记记录 P4 的客户端配置基线重构。以下内容**修订** §2.1 / §3.1 / §3.3 / §3.6，并**修正** P3 补记中 FakeIP 的部分表述（见下），不改变 §2.3 的 D1–D6 决策，Status 仍为 Accepted。

#### CN 分流基线（默认行为变化）

- **基线来源**：内置 `singbox_template` 对齐 GUI.for.SingBox 默认配置，新增 `core_config/baseline.rs` 作为单一真值源（DNS / 路由规则与远程规则集注册）。
- **DNS 基线**：
  - `local` = DoH（`https` `223.5.5.5:443`），无 `detour`（默认直连）；
  - `remote` = DoH（`https` `8.8.8.8:443`），`detour` = 主 selector tag（从生成出站读取，不硬编码；2026-09 由 DoT 853 改为 DoH 443，与代理上的 HTTPS 流量同路径，对齐参考模板）；
  - `dns.rules` = `clash_mode direct → local` / `clash_mode global → remote` / `geosite-cn → local`；`dns.final = remote`；
  - `inject_android_dns` 同步该 DNS 形态并复用同一 DNS 规则与规则集注册。
- **路由基线**：`route.rules` 注入 `geosite-private` / `geosite-cn` / `geoip-private` / `geoip-cn` → `direct`，`geolocation-!cn` → 主 selector。基线位于模板**末尾**，compose（MITM 白名单）、local_override（本地规则）、panel_features（`clash_mode` 模式规则）均**前插**，用户规则优先级始终高于基线。
- **远程规则集**：`route.rule_set` 幂等注册 5 个 MetaCubeX jsDelivr 远程规则集（`geosite-private` / `geoip-private` / `geosite-cn` / `geoip-cn` / `geolocation-!cn`，`format: binary`）；2026-09 起统一引用顶层 `http_clients.rule-set-direct`（无 detour = 绕过路由直连拨号，`domain_resolver` 走直连 `local` DNS）下载，冷启动不再依赖代理可用。
- **默认行为变化**：客户端默认链路从「全量代理」改为「CN 直连分流」——CN / 私有目的地直连，非 CN 走主 selector；`route.final` 仍为主 selector。（已获项目所有者确认。）
- **启动风险（重要，已缓解）**：sing-box 在启动阶段**同步下载**远程规则集，URL 不可达会导致**核心启动失败**（已对 sing-box 1.14 实测；早期「下载失败优雅降级」的描述错误）。2026-09 补记后有两层兜底：①规则集经 `rule-set-direct` HTTP client **直连**下载（不再走 `route.final` 代理），只需 jsDelivr（`testingcf.jsdelivr.net`）直连可达；②`apply_panel_features` 在配置引用远程规则集时注入 `experimental.cache_file`（realip 模式同样注入，不再仅限 FakeIP），sing-box 1.14 起成功下载过的规则集下次启动直接从缓存恢复。

#### FakeIP 架构修正定案

- **定案**：FakeIP 仅作用于**非 CN A 查询**——`{rule_set: ["geolocation-!cn"], query_type: ["A"], action: "route", server: "fakeip"}`，取代此前 `{geosite-cn, invert, A}` 形态。非 CN 域名端到端以**域名**交给出站（核心按域名路由，而非本地解析的真实 IP）；CN 域名由基线的 `geosite-cn → local` 走国内解析器连真实 IP。
- **修正 P3 补记**：P3「FakeIP 覆盖全部 A 查询且不依赖规则集」的表述已被本定案取代——FakeIP 现依赖 `geolocation-!cn` 规则集（基线注册）做非 CN 限定。
- **规则顺序**：丢弃规则（`HTTPS` / `SVCB`，IPv6 关闭时并入 `AAAA` → `predefined` + `NOERROR`）置 `dns.rules` **头部**；FakeIP 路由规则**追加在基线之后**，使 `clash_mode direct/global` 与 `geosite-cn` 优先，FakeIP 不泄漏进直连 / 全局模式。
- **FakeIP 模式无 `resolve`，realip 模式恢复 `resolve`（2026-09 补记）**：FakeIP 模式确认不注入 `route.rules` 的 `{"action":"resolve"}`——该动作会让出站看到真实 IP、违背 FakeIP 目的，并使所有连接依赖代理 DoH。但此前的移除同时误伤了默认 realip 模式：mixed 入站的域名连接没有目的 IP，`geoip-cn` / `geoip-private` 等 IP 规则集无法匹配，未收录国内域名错走代理。realip 模式（FakeIP 关闭且非 Takeover）恢复在 `hijack-dns` 之后注入 `{"action":"resolve"}`（IPv6 关闭时带 `strategy: ipv4_only`），对齐参考模板 realip.json；TUN 连接自带真实目的 IP，resolve 对非 Fqdn 目标为 no-op，不增加 TUN 路径延迟。

- **DNS 可视化映射内置默认，编辑即接管（2026-09 补记）**：内置默认 DNS 以切片 schema 形态经
  `builtin_dns_slice_get` 暴露给前端（`core_config::baseline::builtin_dns_slice` 为真值源，与运行时
  注入逐项对齐：local/remote 服务器、头部 HTTPS/SVCB(+AAAA) 丢弃规则、clash_mode×2、geosite-cn、
  final/strategy/reverse_mapping）；`follow_system` 模式下编辑器直接以内置默认初始化草稿，保存时内容
  与内置一致则保持跟随系统（正文清空）、有改动则自动落 `takeover`——用户无需理解/手动切换 D1 的
  模式开关（模式字段保留于数据模型，仅 UX 自动化）。为支撑内置规则的无损映射，切片 schema 新增
  `clash_mode` 匹配类型（渲染为字符串，目标限 rule/global/direct）与 `reverse_mapping` 字段。FakeIP
  开关仍是设置页独立层，不属于该视图。
- 其余（`cache_file` 合并、`takeover` 豁免、幂等）不变。

#### 冗余开关移除（内容驱动注入）

- **切片级开关移除**：DNS / outbounds / experimental / route 四个切片的 `enabled` 总开关全部删除；注入改为内容驱动——DNS 仅 `mode == takeover` 注入、outbounds 注入 `enabled` 条目、experimental 仅 `cache_file.enabled` 合并、route 仅在 `final_tag` / `resolver` 非空时覆写。
- **规则总开关移除**：`CoreLocalOverride.enabled`（`singbox.enabled`）删除，规则卡片与规则集引用各带 `enabled`，注入逐项决定；移除 `local_override_enabled` 门控参数与 `apply_config_slices` / `build_core_config_v2` 的门控签名。
- **DNS mode 跨平台统一**：`dns_mode_from_slices` 只看 `mode`；桌面不再恒按 takeover 处理——`follow_system` 在桌面保留模板 DNS、Android 保留 `inject_android_dns`，`takeover` 两端均注入切片正文。
- **存储版本**：`config_slices.json` version 1→2（`SLICE_VERSION = 2`）；`load` 幂等折叠旧开关意图并写回。
- **Tauri 契约兼容**：`CoreLocalOverrideView.enabled` 恒 `true` 兼容输出，`CoreLocalOverrideInput.enabled` 接受但忽略。

**迁移矩阵**：

| 旧字段（v1 / 旧 schema） | 旧值 | 折叠后的 v2 语义 |
|---|---|---|
| `config_slices.dns.enabled` | `false` | `dns.mode = follow_system` |
| `config_slices.outbounds.enabled` | `false` | 所有 `items[*].enabled = false` |
| `config_slices.route.enabled` | `false` | 清空 `final_tag` + `resolver` |
| `config_slices.experimental.enabled` | `false` | `cache_file.enabled = false` |
| `local_override.singbox.enabled` | `false` | 所有 `rules[*].enabled = false` + `rule_sets[*].enabled = false` |

- 迁移仅在旧值**显式为 `false`** 时折叠；字段缺失 / `true` 不改动。写回后旧字段不再序列化，二次 `load` 幂等。

#### 基线视图（只读）

- **命令**：新增只读 Tauri 命令 `baseline_view_get`，返回 `BaselineView`（`core_config/view.rs`）。
- **单一真值源**：视图不复制真值——规则集 tag / URL 取自 `CN_RULE_SETS`，路由 / DNS 规则由 `cn_baseline_route_rules` / `cn_baseline_dns_rules` 的实际输出派生，出站 tag / kind 引用基线常量，避免与 `singbox_template` 注入逻辑产生双真相源漂移。
- **静态基线**：视图仅反映静态基线，**不包含**条件性 FakeIP 内容（FakeIP 为 opt-in）。
- **前端**：移动端规则管理 / 规则集 / 出站三页底部新增「内置」只读分区（`BaselineSections.tsx`），与用户可编辑列表分区展示，不可编辑 / 删除。
