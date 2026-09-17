//! 客户端核心配置（桌面客户端核心库）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use pp_common::PanelResult;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(test)]
mod tests;

/// MITM 代理的客户端视图配置。
///
/// 对应 `pp_mitm::MitmConfig`，仅保留桌面客户端需要持久化的字段。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MitmClientConfig {
    /// 存放 MITM CA（`ca.crt` / `ca.key`）的目录，默认 `data_dir/certs`。
    pub ca_dir: PathBuf,
    /// 需要拦截的主机名列表，空列表表示全拦截。
    pub hostnames: Vec<String>,
    /// 脚本钩子使用的脚本方言。
    pub script_dialect: pp_script::ScriptDialect,
}

impl Default for MitmClientConfig {
    fn default() -> Self {
        Self {
            ca_dir: PathBuf::new(),
            hostnames: Vec::new(),
            script_dialect: pp_script::ScriptDialect::Surge,
        }
    }
}

/// 桌面客户端核心配置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientConfig {
    /// 数据目录（配置、证书、核心二进制等统一存放于此）。
    pub data_dir: PathBuf,
    /// Hub 地址，如 `http://127.0.0.1:50052`。
    pub hub_url: String,
    /// 订阅 token。
    pub sub_token: String,
    /// 首页选中的生效订阅（`data_dir/subscriptions.json` 中的订阅 id）；`None` = 未选中。
    ///
    /// 运行模型：订阅 `enabled` 表示「可被首页选择」，此处为当前选中的生效订阅，
    /// 唯一生效。`#[serde(default)]` 保证旧版 `client.json`（无此字段）可正常
    /// 反序列化。
    #[serde(default)]
    pub active_subscription_id: Option<Uuid>,
    /// 核心二进制路径。
    ///
    /// 客户端仅支持 sing-box 核心；旧版 `client.json` 的 `core_type` 字段在反序列化时被
    /// 忽略，`core_type: "mihomo"` 的存量配置在 [`Self::load`] 中一次性归一化（见 load）。
    pub core_binary: PathBuf,
    /// 本地 mixed 入站端口。
    pub mixed_port: u16,
    /// 是否启用 MITM。
    pub mitm_enabled: bool,
    /// MITM 配置（客户端视图）。
    pub mitm: MitmClientConfig,
    /// 是否启用系统代理。
    pub system_proxy_enabled: bool,
    /// 是否启用 TUN 虚拟网卡（需要 root/管理员权限）。
    pub tun_enabled: bool,
    /// TUN 协议栈：mobile `go`（默认，sing-tun 自研栈）/ `mixed` / `system`；
    /// desktop 另保留 `gvisor`（官方核心二进制编入 gVisor，设置页仍提供）。
    pub tun_stack: String,
    /// TUN 自动路由（默认开启）。
    pub tun_auto_route: bool,
    /// 是否启用 IPv6 解析（默认关闭）。
    ///
    /// 关闭时配置合成阶段把 `dns.strategy` 覆写为 `ipv4_only`，避免域名解析出 AAAA 记录后
    /// 经无 IPv6 出栈的节点连接失败；开启时保留模板/注入的 `prefer_ipv4`。DNS 切片 takeover
    /// 模式下由用户全权接管，不受该开关影响。
    /// `#[serde(default)]` 保证旧版 `client.json`（无此字段）解析为 false。
    #[serde(default)]
    pub ipv6_enabled: bool,
    /// 是否启用内置 DNS FakeIP 模式（默认关闭，opt-in）。
    ///
    /// 开启时配置合成阶段（`core_config::apply_singbox_panel_features`）注入 fakeip DNS server
    /// 并把全部 A 查询路由到 fakeip；HTTPS/SVCB 记录丢弃，AAAA 仅在 `ipv6_enabled=false` 时一并
    /// 丢弃。`experimental.cache_file` 被深合并开启以在重启后保留映射。DNS 切片 takeover 模式下
    /// 由用户全权接管，不受该开关影响。
    /// `#[serde(default)]` 保证旧版 `client.json`（无此字段）解析为 false。
    #[serde(default)]
    pub dns_fakeip_enabled: bool,
    /// 是否启用 Clash 面板 API（RESTful 控制接口）。
    ///
    /// 默认开启：流量统计与出站模式即时切换依赖该本地接口。存量 `client.json` 中
    /// 已持久化的显式值在反序列化时优先（尊重用户选择），该默认仅作用于缺失此字段
    /// 或首装无配置文件的新配置。
    pub clash_api_enabled: bool,
    /// Clash 面板 API 监听端口。
    pub clash_api_port: u16,
    /// Clash 面板 API 密钥（空串 = 不鉴权，合成配置时省略该字段）。
    pub clash_api_secret: String,
    /// Clash 面板 UI 选择：`yacd` / `zashboard` / `metacubexd`（默认 `zashboard`）。
    ///
    /// 未知值在配置合成（`core_config::apply_panel_features*`）时回退为 `zashboard`。
    pub clash_api_ui: String,
    /// GitHub 代理前缀（如 `https://gh-proxy.com`）：GitHub 资源 URL 将拼接为该前缀代理；
    /// 空串 = 直连 GitHub。
    pub github_proxy_prefix: String,
    /// 远程资源拉取是否经本地核心 mixed 端口（`http://127.0.0.1:{mixed_port}`）代理。
    pub fetch_via_local_proxy: bool,
    /// Rule mode: `rule` / `global` / `direct` (default `rule`).
    ///
    /// Persisted to client.json. sing-box mode is only the route rule `clash_mode` matching value:
    /// when Clash API is enabled the composed config writes it to `experimental.clash_api.default_mode`
    /// plus baseline `clash_mode` rules (`core_config::apply_singbox_panel_features`), with the
    /// runtime Clash API `PATCH /configs` push as a secondary idempotent path. Illegal values fall
    /// back to `rule` on the read side ([`Self::normalized_rule_mode`]). Struct-level `#[serde(default)]`
    /// ensures old version `client.json` (without this field) parses normally.
    pub rule_mode: String,
    /// Persisted group selections: group name -> selected node name.
    ///
    /// Written when user manually selects a node in a group; replayed after core startup
    /// (see [`crate::proxies::replay_group_selections`]). `#[serde(default)]` ensures backward
    /// compatibility with old `client.json` that lacks this field.
    #[serde(default)]
    pub group_selections: HashMap<String, String>,
    /// Whether to show upload/download traffic in the Android VPN notification.
    /// `#[serde(default)]` ensures backward compatibility with old `client.json`.
    #[serde(default = "default_true")]
    pub vpn_notify_show_traffic: bool,
    /// Whether to show current proxy group & node in the Android VPN notification.
    /// `#[serde(default)]` ensures backward compatibility with old `client.json`.
    #[serde(default = "default_true")]
    pub vpn_notify_show_selection: bool,
}

fn default_true() -> bool {
    true
}

/// gVisor 选项清理（Android）后的存量迁移映射：`gvisor` → `go`。
///
/// gVisor 选项已从移动端 UI 移除，sing-tun 自研栈（`go` / `stack` 缺省，纯用户态）
/// 接替其「无 system 栈设备兼容问题」的回退角色。`is_android` 参数化以便在宿主机
/// 测试 Android 分支（[`ClientConfig::load`] 传 `cfg!(target_os = "android")`）；
/// desktop 仍提供 gVisor 选项（官方核心二进制编入 gVisor），不迁移。
fn migrated_tun_stack(is_android: bool, stack: &str) -> Option<&'static str> {
    if is_android && stack == "gvisor" {
        Some("go")
    } else {
        None
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::new(),
            hub_url: String::new(),
            sub_token: String::new(),
            active_subscription_id: None,
            core_binary: PathBuf::new(),
            mixed_port: 17890,
            mitm_enabled: true,
            mitm: MitmClientConfig::default(),
            system_proxy_enabled: false,
            tun_enabled: false,
            // Android 默认 go（sing-tun 自研栈）：panelcore 升级 sing-box 1.15 后
            // `stack` 缺省即自研栈（纯用户态，无 system 栈在部分 Android 设备上
            // TCP 全灭的兼容问题，也无 gVisor 的性能损耗）；桌面保持 mixed。
            tun_stack: if cfg!(target_os = "android") {
                "go"
            } else {
                "mixed"
            }
            .to_string(),
            tun_auto_route: true,
            // IPv6 默认关闭：DNS 策略 ipv4_only，规避无 v6 出栈节点的 AAAA 连接失败。
            ipv6_enabled: false,
            // FakeIP 默认关闭（opt-in）：仅显式开启时才注入 fakeip DNS。
            dns_fakeip_enabled: false,
            // Clash API 默认开启：流量统计与出站模式即时切换依赖该本地控制接口。
            // 该默认仅影响新配置（首装无 client.json 时）；存量 client.json 中已持久化的
            // 值不受影响（显式字段优先于默认值，desktop 用户显式关闭的尊重选择，
            // mobile 用户可在设置页自行开启/关闭）。
            clash_api_enabled: true,
            clash_api_port: 9090,
            clash_api_secret: String::new(),
            clash_api_ui: "zashboard".to_string(),
            github_proxy_prefix: String::new(),
            fetch_via_local_proxy: false,
            rule_mode: "rule".to_string(),
            group_selections: HashMap::new(),
            vpn_notify_show_traffic: true,
            vpn_notify_show_selection: true,
        }
    }
}

impl ClientConfig {
    /// 基于数据目录构造配置，`ca_dir` 默认指向 `data_dir/certs`。
    pub fn new(
        data_dir: PathBuf,
        hub_url: impl Into<String>,
        sub_token: impl Into<String>,
        core_binary: PathBuf,
    ) -> Self {
        let mut cfg = Self {
            data_dir,
            hub_url: hub_url.into(),
            sub_token: sub_token.into(),
            core_binary,
            ..Self::default()
        };
        cfg.mitm.ca_dir = cfg.data_dir.join("certs");
        cfg
    }

    /// 配置文件路径：`data_dir/client.json`。
    pub fn config_file(&self) -> PathBuf {
        self.data_dir.join("client.json")
    }

    /// 归一化规则模式：合法值 `rule` / `global` / `direct` 原样返回，非法值
    /// （含空串）回退 `"rule"`。
    pub fn normalized_rule_mode(&self) -> &str {
        match self.rule_mode.as_str() {
            "rule" | "global" | "direct" => &self.rule_mode,
            _ => "rule",
        }
    }

    /// 从 `data_dir/client.json` 加载配置。
    ///
    /// 一次性归一化（存量数据迁移，结果写回磁盘）：
    /// - 旧版配置 `core_type: "mihomo"` 时，客户端已仅支持 sing-box——忽略该字段；
    ///   若 `core_binary` 指向 mihomo 二进制则重置为空（交由核心管理自动选择），
    ///   同时剔除 `core_type` 字段；
    /// - Android 端 `tun_stack: "gvisor"`（gVisor 选项清理前的存量值）迁移为 `go`
    ///   （见 [`migrated_tun_stack`]）。
    pub fn load(data_dir: &Path) -> PanelResult<Self> {
        let path = data_dir.join("client.json");
        let text = std::fs::read_to_string(&path)?;
        let mut raw: serde_json::Value = serde_json::from_str(&text)?;
        let legacy_mihomo = raw.get("core_type").and_then(|v| v.as_str()) == Some("mihomo");
        if legacy_mihomo {
            let mihomo_binary = raw
                .get("core_binary")
                .and_then(|v| v.as_str())
                .is_some_and(|p| p.to_lowercase().contains("mihomo"));
            if mihomo_binary {
                raw["core_binary"] = serde_json::Value::String(String::new());
            }
            // 写回读取路径（不能用 `Self::save`：其目标取自配置内的 data_dir 字段）。
            if let Some(obj) = raw.as_object_mut() {
                obj.remove("core_type");
            }
            std::fs::write(&path, serde_json::to_string_pretty(&raw)?)?;
            tracing::info!("旧版 mihomo 配置已归一化为 sing-box 并写回 client.json");
        }
        if let Some(stack) = raw.get("tun_stack").and_then(|v| v.as_str())
            && let Some(next) = migrated_tun_stack(cfg!(target_os = "android"), stack)
        {
            raw["tun_stack"] = serde_json::Value::String(next.to_string());
            // 写回读取路径（不能用 `Self::save`：其目标取自配置内的 data_dir 字段）。
            std::fs::write(&path, serde_json::to_string_pretty(&raw)?)?;
            tracing::info!("存量 tun_stack=gvisor 已迁移为 go 并写回 client.json");
        }
        Ok(serde_json::from_value(raw)?)
    }

    /// 将配置保存到 `data_dir/client.json`。
    pub fn save(&self) -> PanelResult<()> {
        let path = self.config_file();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, text)?;
        Ok(())
    }
}
