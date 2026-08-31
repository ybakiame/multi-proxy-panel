# ADR-0002: 客户端规则管理交互重设计（本地 Override 层 + 规则卡片 + 规则集订阅）

- **Status:** Proposed
- **Date:** 2026-08-26
- **Deciders:** ProxyPanel Contributors
- **Scope:** 阶段④ — 客户端规则管理交互重设计（基于 `docs/research/client-audit-2026-08.md` 阶段③决策）

---

## 1. Context & Problem Statement

### 1.1 现状

ProxyPanel 客户端（`pp-client` + `apps/desktop` + Android）当前规则管理存在以下痛点：

1. **无结构化本地规则层**：Profile 覆写仅支持 YAML/JS 两种高级模式（`profile::ProfileOverrides`），对普通用户门槛过高；没有可视化规则列表。
2. **规则不可见不可管**：`build_core_config_v2` 生成的模板中 `route.rules` 为空（sing-box）或仅 `MATCH,proxy`（mihomo），所有分流依赖订阅自带规则，用户无法插入自定义规则。
3. **规则集管理缺位**：没有内置社区规则集市场，用户需手动编写 `rule_set` / `rule-providers` 语法，双核心差异（sing-box `rule_sets` vs mihomo `rule-providers`）直接暴露给用户。
4. **移动端体验差**：Android 已收敛为 sing-box 主核 + mihomo 静默 fallback，但规则管理仍沿用桌面端的 YAML/JS 覆写模式，无原生交互。

### 1.2 调研结论

`docs/research/client-audit-2026-08.md` 阶段③决策要求：

> - 本地 Override 层设计（schema + 合并策略 + UI）
> - 规则卡片列表 + 场景模板（移动端优先，桌面端复用）
> - mihomo Kotlin 侧补全或冻结：若走 sing-box 主核路线，`MihomoVpnService` 标记为 fallback 维护模式

参考项目调研结论：

| 项目 | 借鉴点 |
|---|---|
| **husi** | 规则编辑 Sheet、路由规则摘要、规则集自动更新、拖拽排序 + 滑动删除 + 撤销 |
| **FlClash** | 三级覆写模式（standard/script/custom）、全局规则库跨配置复用、渐进式 Sheet 导航 |
| **Karing/Streisand** | sing-box 单核的稳定性收益（验证收敛策略） |

---

## 2. Decision

引入**分层渐进式规则管理**架构，包含三个层次：

1. **场景模板层**（最高层）：回国模式 / 海外模式 / 广告过滤，一键生成结构化规则集。
2. **可视化规则卡片层**（中间层）：用户可增删改查的本地规则列表，结构化存储，双核心统一抽象。
3. **JSON/YAML/JS 兜底层**（最底层）：保留现有 Profile 覆写能力，作为高级用户逃生舱口。

**核心原则**：结构化规则优先生成 → YAML/JS 仍作高级兜底 → 合成时本地规则插入在订阅规则之前。

---

## 3. Detailed Design

### 3.1 本地 Override 层 Schema 设计

#### 3.1.1 存储模型：`LocalOverride`（Rust 结构体）

新增 `crates/pp-client/src/local_override/mod.rs`，定义以下核心类型：

```rust
/// 本地 Override 总容器，按核心类型隔离存储（因为规则语法差异无法完全抹平）。
/// 存储位置：`data_dir/local_override.json`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalOverride {
    /// sing-box 侧本地规则与规则集
    #[serde(default)]
    pub singbox: CoreLocalOverride,
    /// mihomo 侧本地规则与规则集
    #[serde(default)]
    pub mihomo: CoreLocalOverride,
    /// 规则集订阅清单（核心无关，统一维护）
    #[serde(default)]
    pub rule_set_subscriptions: Vec<RuleSetSubscription>,
    /// 场景模板应用记录（用于回显/撤销）
    #[serde(default)]
    pub applied_templates: Vec<AppliedTemplate>,
}

/// 单核心的本地 Override
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreLocalOverride {
    /// 用户自定义规则卡片列表（有序）
    #[serde(default)]
    pub rules: Vec<LocalRule>,
    /// 规则集引用列表（有序，对应 rule_sets / rule-providers）
    #[serde(default)]
    pub rule_sets: Vec<LocalRuleSetRef>,
    /// 是否启用本地 Override（总开关）
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// 单条本地规则（双核心统一抽象，渲染时按核心翻译）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalRule {
    /// 规则唯一 ID（前端生成 UUID v4）
    pub id: String,
    /// 用户自定义名称（可选，空则自动生成摘要）
    #[serde(default)]
    pub name: String,
    /// 是否启用
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 规则匹配类型
    pub match_type: RuleMatchType,
    /// 匹配目标（根据 match_type 语义变化）
    pub target: String,
    /// 路由动作
    pub action: RuleAction,
    /// 高级选项
    #[serde(default)]
    pub advanced: RuleAdvancedOptions,
    /// 用户备注
    #[serde(default)]
    pub note: String,
    /// 创建时间戳（用于排序稳定性）
    pub created_at: u64,
    /// 排序权重（越小越靠前）
    pub sort_order: i32,
}

/// 规则匹配类型（与 sing-box route rule 字段对齐，mihomo 侧翻译）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleMatchType {
    /// 精确域名匹配（sing-box: `domain` / mihomo: `DOMAIN`）
    Domain,
    /// 域名后缀匹配（sing-box: `domain_suffix` / mihomo: `DOMAIN-SUFFIX`）
    DomainSuffix,
    /// 域名关键词匹配（sing-box: `domain_keyword` / mihomo: `DOMAIN-KEYWORD`）
    DomainKeyword,
    /// IP CIDR（sing-box: `ip_cidr` / mihomo: `IP-CIDR`/`IP-CIDR6`）
    IpCidr,
    /// 源 IP CIDR（sing-box: `source_ip_cidr` / mihomo: `SRC-IP-CIDR`）
    SourceIpCidr,
    /// 规则集引用（sing-box: `rule_set` / mihomo: `RULE-SET`）
    RuleSet,
    /// Android 应用包名（sing-box: `package_name` / mihomo: `PROCESS-NAME` 近似）
    #[cfg(target_os = "android")]
    AppPackage,
    /// 进程名（桌面端，sing-box: `process_name` / mihomo: `PROCESS-NAME`）
    #[cfg(not(target_os = "android"))]
    ProcessName,
    /// 端口范围（sing-box: `port` / mihomo: `DST-PORT`）
    Port,
    /// 最终兜底规则（sing-box: `final` 在 route.final / mihomo: `MATCH`）
    /// 注：final 规则在列表中仅作为占位，实际写入核心配置的 route.final / MATCH
    Final,
}

/// 路由动作
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleAction {
    /// 代理（route 到主 selector / proxy group）
    Proxy,
    /// 直连
    Direct,
    /// 拒绝
    Reject,
    /// 路由到指定出站标签（高级，需校验目标存在性）
    Outbound { tag: String },
}

/// 高级选项
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleAdvancedOptions {
    /// 跳过 DNS 解析（仅 domain 类规则有效）
    /// sing-box: 规则对象无此字段，通过 `domain`+`ip_cidr` 分离实现语义
    /// mihomo: `no-resolve` 标记
    #[serde(default)]
    pub no_resolve: bool,
    /// 反选（invert match）
    /// sing-box: `invert: true` / mihomo: 通过逻辑规则 `NOT` 模拟
    #[serde(default)]
    pub invert: bool,
    /// 协议 sniff（sing-box 1.12+ action: sniff，暂不暴露给用户）
    #[serde(skip)]
    pub _sniff: bool,
}

/// 规则集引用（本地已下载或远程订阅）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalRuleSetRef {
    pub id: String,
    /// 规则集名称（用户可见）
    pub name: String,
    /// 规则集标签（核心配置中引用）
    pub tag: String,
    /// 规则集类型
    pub kind: RuleSetKind,
    /// 本地文件路径（已下载缓存）或远程 URL
    pub source: RuleSetSource,
    /// 是否启用
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 自动更新间隔（分钟，0 = 不自动更新）
    #[serde(default)]
    pub auto_update_interval_minutes: u32,
    /// 最后更新时间戳
    #[serde(default)]
    pub last_updated: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleSetKind {
    /// sing-box: `remote` rule_set (binary .srs)
    SingBoxRemote,
    /// sing-box: `local` rule_set (source .json)
    SingBoxLocal,
    /// mihomo: `http` rule-provider
    MihomoHttp,
    /// mihomo: `file` rule-provider
    MihomoFile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleSetSource {
    /// 远程 URL（需下载缓存）
    Remote { url: String },
    /// 本地文件路径（已下载）
    Local { path: String },
    /// 内置 bundled 资源（随 App 分发）
    Bundled { name: String },
}

/// 规则集订阅条目（统一清单，核心无关）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleSetSubscription {
    pub id: String,
    /// 社区规则集标识（如 `geoip-cn`, `geosite-ads`, `geosite-cn`）
    pub community_id: String,
    /// 显示名称
    pub display_name: String,
    /// 分类标签
    pub category: RuleSetCategory,
    /// 是否已勾选订阅
    #[serde(default)]
    pub subscribed: bool,
    /// sing-box 远程 URL 模板（含 `{tag}` 占位）
    pub singbox_url_template: String,
    /// mihomo 远程 URL 模板（含 `{tag}` 占位）
    pub mihomo_url_template: String,
    /// 默认自动更新间隔（分钟）
    pub default_interval_minutes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleSetCategory {
    Geoip,
    Geosite,
    Ads,
    Privacy,
    Malware,
    Custom,
}

/// 已应用场景模板记录
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliedTemplate {
    pub template_id: String,
    pub applied_at: u64,
    /// 应用时生成的规则 ID 列表（用于一键撤销）
    pub generated_rule_ids: Vec<String>,
}
```

#### 3.1.2 与现有 Profile 覆写（YAML/JS）的关系

```
配置合成链路（新）：

订阅内容 → 提取节点
    ↓
本地模板生成（singbox_template / mihomo_template）
    ↓
【新增】本地 Override 层注入（结构化规则优先生成）
    - 若 LocalOverride.enabled = true:
      - 规则集引用 → 生成 rule_sets / rule-providers 配置段
      - 本地规则卡片 → 按顺序生成 route.rules / rules 数组
      - 插入位置：订阅规则之前（本地规则优先匹配）
    - 若 enabled = false: 跳过此层
    ↓
Profile YAML 覆写（remote + local deep-merge）
    ↓
Profile JS 覆写（remote + local chained）
    ↓
MITM chain 注入（compose_singbox_config / compose_mihomo_config）
    ↓
PanelFeatures 强制注入（TUN / Clash API）
    ↓
最终核心配置
```

**关键决策**：
- 结构化本地 Override 在 **Profile YAML/JS 之前** 注入，确保可视化规则不会被 YAML/JS 意外覆盖。
- YAML/JS 仍保留为**高级兜底**：高级用户可通过 YAML 直接写 `route.rules` 或 JS 操作 config 对象，实现结构化规则无法覆盖的极端场景。
- 若 YAML/JS 也操作了 `route.rules`，按 RFC 7386 deep-merge 语义，YAML 会递归合并（数组替换），JS 会完全覆盖——这是预期行为，文档中需明确说明层级关系。

#### 3.1.3 合成进核心配置的合并策略

**sing-box 侧**（`pp-client/src/core_config/singbox.rs` 新增 `apply_local_override`）：

```rust
/// 将 LocalOverride 注入 sing-box 配置。
///
/// 注入点：在 `compose_singbox_config` 之后、`apply_panel_features` 之前。
/// 注入策略：
/// 1. `rule_sets`: 追加到 `route.rule_sets`（remote rule_set 数组）
/// 2. `rules`: 插入到 `route.rules` 数组**头部**（本地规则优先于订阅规则）
/// 3. `final`: 若存在 Final 类型规则，写入 `route.final`
pub fn apply_singbox_local_override(
    config: &mut serde_json::Map<String, Value>,
    override: &CoreLocalOverride,
) {
    // ... 实现细节见代码注释
}
```

**mihomo 侧**（`pp-client/src/core_config/mihomo.rs` 新增 `apply_mihomo_local_override`）：

```rust
/// 将 LocalOverride 注入 mihomo 配置。
///
/// 注入点：在 `compose_mihomo_config` 之后、`apply_panel_features` 之前。
/// 注入策略：
/// 1. `rule-providers`: 写入顶层 `rule-providers` 映射
/// 2. `rules`: 插入到 `rules` 数组**头部**，规则集引用生成 `RULE-SET,<tag>,<action>` 格式
/// 3. `MATCH`: 若存在 Final 类型规则，替换最后的 `MATCH,proxy` 或追加
```

**插入顺序保证**：

```
route.rules（sing-box）/ rules（mihomo）数组顺序：

[0] 本地规则卡片 #1（最高优先级）
[1] 本地规则卡片 #2
... 本地规则卡片 #N
[N] 本地规则集引用 #1（RULE-SET / rule_set）
[N+1] 本地规则集引用 #2
... 本地规则集引用 #M
[N+M] 订阅自带规则（来自模板/覆写）
[...] MATCH / final（兜底）
```

#### 3.1.4 存储位置与版本兼容

- **存储路径**：`data_dir/local_override.json`（与 `client.json` / `profiles.json` / `subscriptions.json` 同级）。
- **版本兼容**：
  - `#[serde(default)]` 全字段兜底：旧版 client 无此文件时，返回 `LocalOverride::default()`（空规则列表、启用状态 true）。
  - 首次写入时自动创建文件。
  - 未来 schema 变更通过 `version` 字段 + 迁移函数处理（类似 `ProfileStoreV2` 的 legacy 迁移策略）。
- **Android 特化**：`AppPackage` 规则类型仅在 Android 构建中可用；桌面端用 `ProcessName` 替代。通过 `#[cfg(target_os = "android")]` 控制。

---

### 3.2 规则卡片交互设计

#### 3.2.1 设计原则

- **移动端优先**：所有交互在 375px 宽度下可用，桌面端通过响应式布局复用同一套组件。
- **渐进式披露**：新建/编辑规则走 Sheet（移动端）/ Modal（桌面端），分步导航：类型选择 → 目标输入 → 高级选项折叠面板。
- **即时反馈**：开关切换、排序变化、删除操作均即时生效（内存状态），退出页面时统一持久化。

#### 3.2.2 卡片摘要格式

参考 husi `RuleEntity.summary()` 的截断策略，每张规则卡片展示三行信息：

```
┌─────────────────────────────────────────┐
│ ≡  [开关]  规则名称（或自动生成摘要）        │
│    domain_suffix: googleapis.com         │
│    → Proxy  [no-resolve]                 │
└─────────────────────────────────────────┘
```

**自动生成摘要规则**（当 `name` 为空时）：
- 第一行：`<match_type>: <target>`（截断 32 字符）
- 第二行：`→ <action>` + 高级标记（`[no-resolve]` / `[invert]`）
- 第三行：用户 `note`（如有）或创建时间

**husi 参考**：husi 的 `summary()` 方法将多条件规则拼接为多行文本，超过 5 行截断为 `...`；我们简化为单条件单卡片（复杂条件拆分为多条规则），降低认知负担。

#### 3.2.3 列表操作

| 操作 | 移动端 | 桌面端 | MVP 实现 |
|---|---|---|---|
| **开关** | 卡片右侧 Switch | 卡片右侧 Switch | v1 必须 |
| **排序** | 拖拽手柄（≡）+ 拖拽排序 | 同上 | v1 用上下移动按钮兜底，拖拽可推迟 |
| **删除** | 左滑/右滑删除 + Snackbar 撤销（5s） | 悬停显示删除按钮 + 确认对话框 | v1 必须（滑动删除移动端优先） |
| **编辑** | 点击卡片 → 底部 Sheet | 点击卡片 → Modal | v1 必须 |
| **批量操作** | 长按进入多选 → 顶部工具栏 | Checkbox 多选 → 顶部工具栏 | v2 |

**滑动删除撤销机制**（参考 husi `RouteScreen.kt`）：
- 滑动删除后，`undoableRemove(ruleId)` 将规则移入 `pending_deletion` 队列。
- Snackbar 显示「已删除 × 条规则 [撤销]」，5 秒内点击撤销则恢复。
- 页面退出（`DisposableEffect.onDispose`）或 5 秒超时后，`commit()` 真正写入删除。

#### 3.2.4 编辑 Sheet/Modal 渐进导航

参考 FlClash `_AddOrEditRuleNestedSheet` 的 PagedSheetRoute 设计，简化为三步：

**Step 1 — 类型选择**：
```
┌─────────────────┐
│ 选择匹配类型      │
│ ─────────────── │
│ ○ 域名 (domain)  │
│ ○ 域名后缀        │
│ ○ IP 段 (CIDR)   │
│ ○ 规则集          │
│ ○ 应用包名        │  ← Android only
│ ○ 最终规则 (final)│
└─────────────────┘
```

**Step 2 — 目标与动作**：
```
┌─────────────────────────────┐
│ 编辑规则                      │
│ ─────────────────────────── │
│ 匹配目标: [googleapis.com   ] │
│                              │
│ 路由动作: [Proxy ▼]           │
│   ○ Proxy  ○ Direct  ○ Reject│
│                              │
│ [保存]                        │
└─────────────────────────────┘
```

**Step 3 — 高级选项（折叠面板，默认收起）**：
```
┌─────────────────────────────┐
│ ▼ 高级选项                    │
│   [✓] 跳过 DNS 解析           │
│   [ ] 反选 (invert)           │
│   备注: [________________]   │
└─────────────────────────────┘
```

**桌面端适配**：Sheet 改为居中 Modal（宽度 480px），内容布局不变。

---

### 3.3 场景模板

#### 3.3.1 模板定义

场景模板是预置的规则组合，用户一键应用后生成多条 `LocalRule` 插入列表。**模板本身不持久化**，仅记录 `AppliedTemplate`（用于回显和撤销）。

| 模板 | 用途 | 生成的规则（sing-box 语义） |
|---|---|---|
| **回国模式** | 海外用户访问国内服务直连 | 1. `domain_suffix: [cn, com.cn, net.cn]` → `direct` <br> 2. `rule_set: geoip-cn` → `direct` <br> 3. `rule_set: geosite-cn` → `direct` |
| **海外模式** | 国内用户访问海外服务代理 | 1. `domain_suffix: [google.com, youtube.com, github.com]` → `proxy` <br> 2. `rule_set: geosite-geolocation-!cn` → `proxy` |
| **广告过滤** | 拦截常见广告域名 | 1. `rule_set: geosite-ads` → `reject` <br> 2. `rule_set: geosite-category-ads-all` → `reject` |
| **局域网直连** | 局域网 IP 段不走代理 | 1. `ip_cidr: [192.168.0.0/16, 10.0.0.0/8, 172.16.0.0/12]` → `direct` <br> 2. `domain: [localhost, <local>]` → `direct` |

#### 3.3.2 模板应用流程

1. 用户点击「应用模板」→ 选择模板 → 确认。
2. 系统生成规则列表，每条规则 `id = UUIDv4()`，`sort_order` 取当前列表最小值 - 100（插入头部）。
3. 记录 `AppliedTemplate { template_id, applied_at, generated_rule_ids }`。
4. 用户可在「已应用模板」区域看到记录，支持「撤销此模板」（删除对应 `generated_rule_ids`）。

#### 3.3.3 模板与规则集订阅的关系

- 模板中引用的 `rule_set`（如 `geoip-cn`、`geosite-ads`）需先在「规则集订阅」中勾选并下载。
- 若规则集未下载，应用模板时提示用户「规则集 geoip-cn 未订阅，是否自动订阅并下载？」→ 自动勾选 + 触发下载。

---

### 3.4 规则集订阅

#### 3.4.1 内置社区规则集清单

预置清单硬编码在 `pp-client` 中（`local_override::BUILT_IN_RULE_SET_SUBSCRIPTIONS`），用户首次进入规则集页面时自动初始化到 `local_override.json`。

| community_id | display_name | category | singbox_url_template | mihomo_url_template |
|---|---|---|---|---|
| `geoip-cn` | GeoIP 中国 | Geoip | `https://github.com/MetaCubeX/meta-rules-dat/raw/sing/geo-lite/ip/cn.srs` | `https://github.com/MetaCubeX/meta-rules-dat/raw/meta/geo-lite/ip/cn.yaml` |
| `geosite-cn` | GeoSite 中国 | Geosite | `https://github.com/MetaCubeX/meta-rules-dat/raw/sing/geo-lite/geosite/cn.srs` | `https://github.com/MetaCubeX/meta-rules-dat/raw/meta/geo-lite/geosite/cn.yaml` |
| `geosite-ads` | 广告域名 | Ads | `https://github.com/MetaCubeX/meta-rules-dat/raw/sing/geo-lite/geosite/category-ads-all.srs` | `https://github.com/MetaCubeX/meta-rules-dat/raw/meta/geo-lite/geosite/category-ads-all.yaml` |
| `geosite-geolocation-!cn` | 非中国地理定位 | Geosite | `https://github.com/MetaCubeX/meta-rules-dat/raw/sing/geo/geosite/geolocation-!cn.srs` | `https://github.com/MetaCubeX/meta-rules-dat/raw/meta/geo/geosite/geolocation-!cn.yaml` |
| `geoip-private` | 私有 IP 段 | Geoip | `https://github.com/MetaCubeX/meta-rules-dat/raw/sing/geo-lite/ip/private.srs` | `https://github.com/MetaCubeX/meta-rules-dat/raw/meta/geo-lite/ip/private.yaml` |

**注**：URL 模板使用 MetaCubeX/meta-rules-dat 社区维护的每日自动构建规则集，sing-box 侧为 `.srs` 二进制格式，mihomo 侧为 `.yaml` 文本格式。

#### 3.4.2 勾选订阅与自动更新

- **UI 呈现**：规则集市场页面，每个规则集展示名称、分类、大小（下载后显示）、更新状态。
- **勾选订阅**：Toggle 开关，开启后自动下载并缓存；关闭后保留本地文件但核心配置中不引用。
- **自动更新间隔**：全局设置（默认 24 小时），支持 1h / 6h / 12h / 24h / 7d / 关闭。
- **下载缓存**：`data_dir/rule_sets/<community_id>.{srs,yaml}`，按核心类型分别存储。
- **失败处理**：下载失败保留旧缓存，记录警告日志，下次启动/定时任务时重试。

#### 3.4.3 双核心适配原则（pp-config 统一处理）

**核心原则**：规则集订阅层完全抽象社区规则集概念，不向用户暴露 `rule_sets` vs `rule-providers` 语法差异。双核心适配由 `pp-client` 在配置合成时翻译。

**sing-box 侧翻译**（`apply_singbox_local_override` 中）：
```json
{
  "route": {
    "rule_sets": [
      {
        "type": "remote",
        "tag": "geoip-cn",
        "format": "binary",
        "url": "https://.../cn.srs",
        "download_detour": "proxy"
      }
    ],
    "rules": [
      { "rule_set": "geoip-cn", "outbound": "direct" }
    ]
  }
}
```

**mihomo 侧翻译**（`apply_mihomo_local_override` 中）：
```yaml
rule-providers:
  geoip-cn:
    type: http
    behavior: ipcidr
    url: "https://.../cn.yaml"
    path: ./rule_sets/geoip-cn.yaml
    interval: 86400

rules:
  - RULE-SET,geoip-cn,direct
```

**pp-config 职责**：`pp-config` crate 负责服务端节点配置生成（Hub ↔ Agent），不涉及客户端规则集。客户端规则集翻译由 `pp-client` 独立处理，避免污染服务端配置生成逻辑。

---

### 3.5 配置合成链路修改点

基于现有 `pp-client/src/state/mod.rs` 的 `start()` 流程，在以下位置插入本地 Override 层：

```rust
// 现有流程（简化）：
let profile_cfg = profile::build_core_config_v2(...).await?;
let chain = self.start_mitm_chain().await?;
// ...
let config_json = match self.config.core_type {
    CoreType::SingBox => {
        let mut cfg = core_config::compose_singbox_config(&profile_cfg, ...)?;
        // 【插入点 A】在此注入本地 Override
        local_override::apply_to_singbox(&mut cfg, &local_ovr.singbox)?;
        core_config::apply_panel_features(&mut cfg, ...);
        cfg
    }
    CoreType::Mihomo => {
        let yaml = serde_yaml::to_string(&profile_cfg)?;
        let mut cfg = core_config::compose_mihomo_config(&yaml, ...)?;
        // 【插入点 B】在此注入本地 Override
        local_override::apply_to_mihomo(&mut cfg, &local_ovr.mihomo)?;
        core_config::apply_panel_features(&mut cfg, ...);
        cfg
    }
};
```

**新增文件清单**（`crates/pp-client/src/local_override/`）：

```
local_override/
├── mod.rs          # 公共类型定义 + 存储加载
├── schema.rs       # LocalRule / RuleMatchType / RuleAction 等类型
├── store.rs        # LocalOverrideStore（文件读写 + 迁移）
├── singbox.rs      # sing-box 侧配置注入（apply_to_singbox）
├── mihomo.rs       # mihomo 侧配置注入（apply_to_mihomo）
├── template.rs     # 场景模板定义 + 应用逻辑
├── ruleset.rs      # 规则集下载 + 缓存 + 自动更新
└── tests/
    ├── mod.rs
    ├── schema.rs   # 类型序列化/反序列化测试
    ├── inject.rs   # 双核心注入逻辑测试
    └── template.rs # 场景模板生成测试
```

---

## 4. MVP 边界（v1 做什么 / 不做什么）

### 4.1 v1 必须实现（P0）

- [ ] `LocalOverride` schema 定义与 `local_override.json` 存储。
- [ ] 规则卡片列表：增删改查、开关、摘要显示。
- [x] 规则编辑 Sheet/Modal：类型选择 → 目标输入 → 动作选择，三步完成。
- [ ] 上下移动按钮排序（拖拽排序推迟到 v2）。
- [ ] 滑动删除 + Snackbar 撤销（移动端）。
- [ ] 场景模板：回国模式 / 海外模式 / 广告过滤 3 个模板。
- [ ] 规则集订阅：内置清单 5 项、勾选订阅、手动更新。
- [ ] 双核心配置注入：`apply_to_singbox` + `apply_to_mihomo`。
- [ ] 与现有 Profile YAML/JS 覆写的层级关系文档化。

### 4.2 v1 不做（推迟到 v2+）

- [ ] **拖拽排序**：用上下移动按钮替代（实现成本低，移动端可用性可接受）。
- [ ] **批量操作**：长按多选、批量删除/开关。
- [ ] **规则搜索/过滤**：列表顶部搜索框。
- [ ] **自定义规则集 URL**：仅支持内置清单，不支持用户输入任意规则集 URL。
- [ ] **规则命中测试**：类似 husi 的「规则集匹配测试」工具。
- [ ] **进程名规则**：桌面端 `ProcessName` 类型（v1 仅支持 Android `AppPackage` 和通用类型）。
- [ ] **JSON Schema 编辑器**：husi 式的 ConfigEditScreen 完整 JSON 编辑器（保留在 Profile YAML/JS 层）。
- [ ] **规则导入/导出**：从剪贴板导入 Surge/Clash 规则语法。

### 4.3 v2 规划（候选）

- 拖拽排序（`@dnd-kit/core` 或原生 HTML5 DnD）。
- 自定义规则集 URL + 分类管理。
- 规则命中测试工具（调用核心 `check` 命令或模拟匹配）。
- 局域网直连模板 + 更多社区模板。
- 规则云同步（与 Hub 用户账户绑定）。

---

## 5. 风险与回退

### 5.1 风险矩阵

| 风险 | 概率 | 影响 | 缓解措施 |
|---|---|---|---|
| **规则集下载失败导致核心启动失败** | 中 | 高 | 下载失败保留旧缓存；首次下载失败时核心配置中不引用该规则集（ graceful degradation）；启动前校验规则集文件存在性。 |
| **本地规则与 YAML/JS 覆写冲突** | 中 | 中 | 明确文档层级关系（本地 Override → YAML → JS）；YAML/JS 仍操作 `route.rules` 时按原有语义覆盖；提供「禁用本地 Override」总开关。 |
| **双核心规则语义差异导致行为不一致** | 中 | 中 | `RuleMatchType` 统一抽象，翻译层单元测试覆盖所有类型；mihomo fallback 场景下规则集自动切换格式（`.srs` → `.yaml`）。 |
| **存储 schema 变更导致旧数据不兼容** | 低 | 中 | `#[serde(default)]` 全字段兜底；引入 `version` 字段预留迁移通道；首次加载失败时回退到空配置（不阻塞启动）。 |
| **Android 包名规则在 mihomo fallback 下失效** | 低 | 中 | mihomo 的 `PROCESS-NAME` 与 Android 包名语义近似但不完全等价；文档说明限制；mihomo 标记为 fallback 维护模式，不新增功能。 |
| **规则集自动更新消耗流量** | 低 | 低 | 默认 24h 间隔；提供「仅 Wi-Fi 下更新」选项（复用现有 `network_type` 感知）；规则集文件体积通常 < 1MB。 |

### 5.2 回退策略

1. **功能级回退**：`CoreLocalOverride.enabled = false` 总开关，一键关闭本地 Override 层，回退到现有纯 Profile 覆写模式。
2. **文件级回退**：`local_override.json` 损坏或不可读时，日志警告并回退到空配置，不阻塞核心启动。
3. **版本级回退**：若 v1 发布后出现严重问题，可通过删除 `local_override.json` 完全回退到阶段③之前的状态（Profile YAML/JS 覆写不受影响）。
4. **核心级回退**：mihomo 侧若规则集注入实现复杂度过高，v1 可先仅支持 sing-box 主核的完整规则管理，mihomo fallback 保持现有行为（仅基础 `MATCH,proxy`）。

---

## 6. UI/UX 参考与组件映射

### 6.1 移动端（React / HeroUI Native）

| 功能 | 参考实现 | 组件建议 |
|---|---|---|
| 规则列表 | husi `RouteScreen.kt` | `FlatList` + 自定义卡片组件 |
| 拖拽排序 | husi `DragDropSwipeLazyColumn` | v1 用上下按钮；v2 用 `@dnd-kit/native` |
| 滑动删除 | husi `SwipeToDismissBox` | `react-native-gesture-handler` Swipeable |
| 编辑 Sheet | FlClash `_AddOrEditRuleNestedSheet` | HeroUI `Sheet` + 内部 `Navigator` 模拟分页 |
| 开关 | husi `Switch` | HeroUI `Switch` |

### 6.2 桌面端（React / HeroUI）

| 功能 | 参考实现 | 组件建议 |
|---|---|---|
| 规则列表 | FlClash `AddedRulesView` | HeroUI `Listbox` / 自定义卡片 |
| 排序 | FlClash `ReorderableList` | v1 用上下按钮；v2 用 `@dnd-kit/sortable` |
| 编辑 Modal | FlClash `AddOrEditRuleDialog` | HeroUI `Modal` + 内部 Stepper |
| 多选 | FlClash `itemsProvider` | HeroUI `Checkbox` + 工具栏 |

---

## 7. 相关文件

- `docs/research/client-audit-2026-08.md` — 阶段③调研结论与行动项
- `crates/pp-client/src/profile/mod.rs` — 现有 Profile 覆写层（YAML/JS）
- `crates/pp-client/src/core_config/compose.rs` — 配置合成入口
- `crates/pp-client/src/core_config/singbox.rs` — sing-box PanelFeatures 注入
- `crates/pp-client/src/core_config/mihomo.rs` — mihomo PanelFeatures 注入
- `crates/pp-client/src/state/mod.rs` — ClientState 启动流程（插入点）
- `.reference/husi/composeApp/.../RouteScreen.kt` — husi 规则列表交互参考
- `.reference/husi/composeApp/.../RouteSettingsScreen.kt` — husi 规则编辑 Sheet 参考
- `.reference/FlClash/lib/views/profiles/overwrite/` — FlClash 三级覆写模式参考
- `.reference/FlClash/lib/views/config/rules.dart` — FlClash 全局规则列表参考
- `.reference/FlClash/lib/views/profiles/overwrite/custom/rules.dart` — FlClash 自定义规则 Sheet 参考

---

## 8. 待办任务（实施拆分）

### 8.1 Rust 后端（pp-client）— ✅ v1 已完成

- [x] `local_override/schema.rs` — 类型定义
- [x] `local_override/store.rs` — JSON 文件存储 + 迁移
- [x] `local_override/singbox.rs` — sing-box 配置注入
- [x] `local_override/mihomo.rs` — mihomo 配置注入
- [x] `local_override/template.rs` — 4 个场景模板定义
- [x] `local_override/ruleset.rs` — 规则集下载 + 缓存 + 自动更新调度
- [x] 修改 `ClientState::start()` 插入本地 Override 注入点
- [x] 单元测试：schema 序列化、双核心注入、模板生成

### 8.2 前端（apps/desktop / apps/android）— ✅ v1 已完成

- [x] 规则列表页面（移动端 Sheet / 桌面 Modal 复用）
- [x] 规则卡片组件（摘要、开关、上下移动、滑动删除）
- [x] 规则编辑 Sheet（类型 → 目标 → 高级三步）
- [x] 场景模板页面（一键应用 + 撤销）
- [x] 规则集订阅页面（清单、勾选、更新状态）
- [x] Tauri / Android 命令桥接（`local_override_load` / `save` / `apply_template` 等）

### 8.3 文档

- [ ] 更新 `docs/architecture.md` — 客户端配置合成链路
- [ ] 更新 `docs/api_reference.md` — 如有新增 Tauri 命令
- [ ] 用户手册 — 规则管理功能说明

---

*本 ADR 基于 `docs/research/client-audit-2026-08.md` 阶段③决策，为阶段④实施提供可落地的技术方案。*
