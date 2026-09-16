//! CN-split baseline: the remote rule-set registry plus the DNS / route rules that make up the
//! GUI.for.SingBox-aligned default chain.
//!
//! The baseline is injected by [`crate::profile::singbox_template`]; the FakeIP injection
//! ([`super::fakeip::apply_fakeip_mode`]) reuses [`ensure_cn_rule_sets`] idempotently.

use serde_json::{Value, json};

use crate::config_slices::{
    DnsMatchType, DnsRule, DnsRuleAction, DnsServer, DnsServerType, DnsSlice, DnsStrategy,
};

/// The built-in (default) DNS configuration as an editable [`DnsSlice`] — the same shape the
/// running core actually uses, mapped into the slice schema so the frontend DNS editor can
/// display and modify it directly instead of treating the built-in DNS as an opaque black box
/// (the "edit the defaults ⇒ takeover" flow, ADR-0005 2026-09 补记).
///
/// Composition mirrors the runtime layering exactly:
///
/// - `servers` = template / `inject_android_dns` pair: `local` (DoH 223.5.5.5:443, direct) and
///   `remote` (DoH 8.8.8.8:443, `detour` = main selector tag `proxy`);
/// - `rules` = the panel-feature drop rule ([`super::fakeip::drop_query_types`]: HTTPS/SVCB,
///   plus AAAA when `ipv6_enabled = false`) at the head, then
///   [`cn_baseline_dns_rules`] (clash_mode direct/global → local/remote, geosite-cn → local);
/// - `final_tag` = `remote`, `strategy` = `prefer_ipv4` (IPv6 on) / `ipv4_only` (IPv6 off,
///   mirroring the panel-feature strategy override), `reverse_mapping` = on.
///
/// FakeIP is deliberately **not** part of this view: it is an opt-in panel feature with its
/// own settings switch, layered on top of this baseline.
///
/// Rule `id`s are stable `builtin-*` strings (not UUIDs) so the frontend can recognize the
/// unmodified default rows; materializing the view as a takeover body keeps them harmless.
#[must_use]
pub fn builtin_dns_slice(ipv6_enabled: bool) -> DnsSlice {
    let mut drop_target = "HTTPS,SVCB".to_string();
    if !ipv6_enabled {
        drop_target.push_str(",AAAA");
    }
    let rule = |id: &str, match_type: DnsMatchType, target: &str, server_tag: &str| DnsRule {
        id: id.to_string(),
        enabled: true,
        match_type,
        target: target.to_string(),
        server_tag: server_tag.to_string(),
        action: DnsRuleAction::Route,
        rcode: String::new(),
    };
    DnsSlice {
        mode: Default::default(),
        servers: vec![
            DnsServer {
                tag: "local".to_string(),
                server: "223.5.5.5".to_string(),
                server_type: DnsServerType::Https,
                server_port: Some(443),
                ..Default::default()
            },
            DnsServer {
                tag: "remote".to_string(),
                server: "8.8.8.8".to_string(),
                server_type: DnsServerType::Https,
                server_port: Some(443),
                detour: OUTBOUND_TAG_PROXY.to_string(),
                ..Default::default()
            },
        ],
        rules: vec![
            DnsRule {
                id: "builtin-drop".to_string(),
                enabled: true,
                match_type: DnsMatchType::QueryType,
                target: drop_target,
                server_tag: String::new(),
                action: DnsRuleAction::Predefined,
                rcode: "NOERROR".to_string(),
            },
            rule(
                "builtin-mode-direct",
                DnsMatchType::ClashMode,
                "direct",
                "local",
            ),
            rule(
                "builtin-mode-global",
                DnsMatchType::ClashMode,
                "global",
                "remote",
            ),
            rule(
                "builtin-geosite-cn",
                DnsMatchType::RuleSet,
                "geosite-cn",
                "local",
            ),
        ],
        final_tag: "remote".to_string(),
        strategy: if ipv6_enabled {
            DnsStrategy::PreferIpv4
        } else {
            DnsStrategy::Ipv4Only
        },
        reverse_mapping: true,
    }
}
///
/// HTTP client tag used for remote rule-set downloads (direct dial, bootstrap-resolved).
///
/// Registered at the top-level `http_clients` array by [`ensure_rule_set_http_client`] and
/// referenced by every CN-split rule set registered in [`ensure_cn_rule_sets`].
pub const RULE_SET_HTTP_CLIENT_TAG: &str = "rule-set-direct";

/// CN-split remote rule-set registry (`tag`, jsDelivr URL), aligned with the GUI.for.SingBox
/// default profile and the MetaCubeX `meta-rules-dat@sing` source family already used by the
/// FakeIP split.
///
/// `geolocation-!cn` lives under the `geosite/` directory (it is a geosite list of non-CN
/// domains); the rest map to their same-named `geosite/` / `geoip/` entry.
pub const CN_RULE_SETS: [(&str, &str); 5] = [
    (
        "geosite-private",
        "https://testingcf.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@sing/geo/geosite/private.srs",
    ),
    (
        "geoip-private",
        "https://testingcf.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@sing/geo/geoip/private.srs",
    ),
    (
        "geosite-cn",
        "https://testingcf.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@sing/geo/geosite/cn.srs",
    ),
    (
        "geoip-cn",
        "https://testingcf.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@sing/geo/geoip/cn.srs",
    ),
    (
        "geolocation-!cn",
        "https://testingcf.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@sing/geo/geosite/geolocation-!cn.srs",
    ),
];

/// Built-in outbound tags (single source of truth shared by [`crate::profile::singbox_template`]
/// and the read-only baseline view [`crate::core_config::BaselineView`]).
pub const OUTBOUND_TAG_PROXY: &str = "proxy";
/// See [`OUTBOUND_TAG_PROXY`].
pub const OUTBOUND_TAG_AUTO: &str = "auto";
/// See [`OUTBOUND_TAG_PROXY`].
pub const OUTBOUND_TAG_DIRECT: &str = "direct";
/// See [`OUTBOUND_TAG_PROXY`].
pub const OUTBOUND_TAG_BLOCK: &str = "block";

/// Built-in outbound `type` values (single source of truth shared by
/// [`crate::profile::singbox_template`] and [`crate::core_config::BaselineView`]).
pub const OUTBOUND_KIND_SELECTOR: &str = "selector";
/// See [`OUTBOUND_KIND_SELECTOR`].
pub const OUTBOUND_KIND_URLTEST: &str = "urltest";
/// See [`OUTBOUND_KIND_SELECTOR`].
pub const OUTBOUND_KIND_DIRECT: &str = "direct";
/// See [`OUTBOUND_KIND_SELECTOR`].
pub const OUTBOUND_KIND_BLOCK: &str = "block";

/// CN-split baseline DNS rules (GUI.for.SingBox default profile): `clash_mode direct → local`,
/// `clash_mode global → remote`, `geosite-cn → local`; `dns.final = remote` handles the rest.
///
/// The clash_mode rules make DNS follow the outbound mode switch (direct mode resolves
/// everything through the domestic resolver, global through the remote one) and deliberately
/// precede the FakeIP rule, so FakeIP never leaks into direct/global mode.
pub fn cn_baseline_dns_rules() -> Vec<Value> {
    vec![
        json!({ "clash_mode": "direct", "action": "route", "server": "local" }),
        json!({ "clash_mode": "global", "action": "route", "server": "remote" }),
        json!({ "rule_set": ["geosite-cn"], "action": "route", "server": "local" }),
    ]
}

/// 内置分组规格（物化进出站切片的真值源，2026-09）。
///
/// 内置分组**可修改不可删除**：物化后用户可调 selector 的 `default` / urltest 的
/// `url` / `interval` / `tolerance` / 两者的 `interrupt_exist_connections`，成员列表
/// 由模板动态计算（订阅节点变化跟随），不可在切片中编辑。id / name 稳定，勿改。
pub struct BuiltinOutboundGroupSpec {
    /// 稳定条目 ID（物化进切片的 `CustomOutbound.id`）。
    pub id: &'static str,
    /// 名称 = 模板分组 tag（apply 时按此匹配模板出站做字段覆写）。
    pub name: &'static str,
    /// 是否 selector（false = urltest）。
    pub selector: bool,
}

/// 内置分组（顺序 = 列表展示顺序：主选择器在前，自动测速在后）。
pub const BUILTIN_OUTBOUND_GROUPS: [BuiltinOutboundGroupSpec; 2] = [
    BuiltinOutboundGroupSpec {
        id: "builtin-group-proxy",
        name: OUTBOUND_TAG_PROXY,
        selector: true,
    },
    BuiltinOutboundGroupSpec {
        id: "builtin-group-auto",
        name: OUTBOUND_TAG_AUTO,
        selector: false,
    },
];

/// 内置路由规则规格（物化进 local_override 统一规则列表的真值源，2026-09）。
///
/// 内置规则**可修改不可删除**（见 `local_override` store 迁移与保存守卫）：物化后用户可
/// 调整 `enabled` / 出站动作与排序位置，`match_type` / `target` 由本规格在保存时归一化。
/// id 稳定（写入 local_override.json），勿改。
pub struct BuiltinRouteRuleSpec {
    /// 稳定规则 ID（物化进 local_override 的 `LocalRule.id`）。
    pub id: &'static str,
    /// 展示名（列表标题；内置规则名称不可改）。
    pub name: &'static str,
    /// 逗号分隔的规则集 tag（渲染为 sing-box `rule_set` 数组）。
    pub target: &'static str,
    /// 默认动作：`true` = direct，`false` = 主 selector（proxy）。
    pub direct: bool,
}

/// 内置路由规则（顺序 = 物化默认排序：CN 直连在前，非 CN 代理兜底在后）。
pub const BUILTIN_ROUTE_RULES: [BuiltinRouteRuleSpec; 2] = [
    BuiltinRouteRuleSpec {
        id: "builtin-cn-direct",
        name: "内置：私有与国内直连",
        target: "geosite-private,geoip-private,geosite-cn,geoip-cn",
        direct: true,
    },
    BuiltinRouteRuleSpec {
        id: "builtin-non-cn-proxy",
        name: "内置：非中国大陆代理",
        target: "geolocation-!cn",
        direct: false,
    },
];

/// CN-split baseline route rules (GUI.for.SingBox default profile): private / CN destinations
/// go `direct`, `geolocation-!cn` (non-CN) goes through the main selector.
///
/// 2026-09 起合并为 2 条：4 个直连规则集塞进同一条规则（sing-box `rule_set` 数组为 OR
/// 语义，与拆分四条等价），非 CN 一条。内置规则物化进 local_override 后，本函数仍作为
/// 物化默认值与 BaselineView 的真值源。
///
/// `proxy_tag` is the main selector outbound tag, read from the generated outbounds rather than
/// hardcoded so the rules follow the template's actual group tag.
pub fn cn_baseline_route_rules(proxy_tag: &str) -> Vec<Value> {
    vec![
        json!({
            "rule_set": ["geosite-private", "geoip-private", "geosite-cn", "geoip-cn"],
            "outbound": "direct"
        }),
        json!({ "rule_set": ["geolocation-!cn"], "outbound": proxy_tag }),
    ]
}

/// Register the CN-split remote rule sets under `route.rule_set` (idempotent by tag).
///
/// Used by [`crate::profile::singbox_template`] (baseline) and by the FakeIP injection
/// ([`super::fakeip::apply_fakeip_mode`]) so a rule referencing `geosite-cn` /
/// `geolocation-!cn` always resolves. Existing entries with the same tag (user / override
/// supplied) are respected and left untouched.
///
/// Note: sing-box downloads remote rule sets synchronously at startup and **fails to start**
/// when a URL is unreachable (verified against sing-box 1.14). Registered entries reference the
/// [`RULE_SET_HTTP_CLIENT_TAG`] HTTP client so downloads dial **directly** (resolved via the
/// direct `local` DNS) instead of being routed through `route.final` (the proxy) — otherwise a
/// cold start would depend on the proxy being up before any rule can even be evaluated (and the
/// proxy itself is what those rules route). `experimental.cache_file` (injected by the panel
/// features stage, see `super::fakeip::ensure_cache_file`) is the offline fallback on top.
pub fn ensure_cn_rule_sets(route: &mut serde_json::Map<String, Value>) {
    let rule_set = route
        .entry("rule_set")
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(arr) = rule_set.as_array_mut() else {
        return;
    };
    for (tag, url) in CN_RULE_SETS {
        if arr
            .iter()
            .any(|rs| rs.get("tag").and_then(Value::as_str) == Some(tag))
        {
            continue;
        }
        arr.push(json!({
            "type": "remote",
            "tag": tag,
            "format": "binary",
            "url": url,
            "http_client": RULE_SET_HTTP_CLIENT_TAG
        }));
    }
}

/// Ensure the top-level `http_clients` array carries the [`RULE_SET_HTTP_CLIENT_TAG`] client:
/// direct dial (no `detour` — HTTP clients bypass the routing system unless one is given) with
/// the download URL's domain resolved by the direct `local` DNS server, so remote rule-set
/// downloads never depend on the proxy path (see [`ensure_cn_rule_sets`]).
///
/// No-op when the config has no `local` DNS server (a fully user-owned DNS body, where
/// hardcoding a resolver reference would risk an invalid config); existing entries with the
/// same tag (user-supplied) are respected.
pub fn ensure_rule_set_http_client(obj: &mut serde_json::Map<String, Value>) {
    let has_local_dns = obj
        .get("dns")
        .and_then(|d| d.get("servers"))
        .and_then(Value::as_array)
        .is_some_and(|servers| {
            servers
                .iter()
                .any(|s| s.get("tag").and_then(Value::as_str) == Some("local"))
        });
    if !has_local_dns {
        tracing::debug!("无 local DNS server，跳过规则集直连 http_client 注入");
        return;
    }
    let clients = obj
        .entry("http_clients")
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(arr) = clients.as_array_mut() else {
        return;
    };
    if arr
        .iter()
        .any(|c| c.get("tag").and_then(Value::as_str) == Some(RULE_SET_HTTP_CLIENT_TAG))
    {
        return;
    }
    arr.push(json!({
        "tag": RULE_SET_HTTP_CLIENT_TAG,
        "domain_resolver": { "server": "local" }
    }));
}
