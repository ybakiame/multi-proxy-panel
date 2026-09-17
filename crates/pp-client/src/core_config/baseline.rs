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
                // 境外解析器：Google 公共 DNS 的 UDP（经代理 UDP 转发）；Google 的 DoH 端点
                // 默认不可通（tls/https 型 8.8.8.8 在国内直连与部分节点落地均被拦），
                // UDP:53 走代理 UDP 中继反而最稳。tag = proxy（2026-09 由 remote 更名，
                // 表达"经代理的解析器"语义）。
                tag: "proxy".to_string(),
                server: "8.8.8.8".to_string(),
                server_type: DnsServerType::Udp,
                server_port: Some(53),
                detour: OUTBOUND_TAG_PROXY.to_string(),
                ..Default::default()
            },
        ],
        // Country/region rule sets (`geosite-cn` and friends) are no longer built-in
        // (2026-09): users add them from the rule-set market and then build their own DNS
        // rules. The built-in default keeps only the mode-driven split (drop + clash_mode)
        // plus the `final` fallback, so it never references a rule set the user may not have.
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
                "proxy",
            ),
        ],
        final_tag: "proxy".to_string(),
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

/// Built-in rule set spec: the source of truth materialized into `custom_rule_sets` (2026-09).
///
/// A built-in rule set **is an ordinary rule set**: once materialized it has exactly the same
/// shape as an entry the user adds from the market (Remote + Binary), so it can be edited,
/// deleted, and restored from this spec by the "reset built-in rule sets" action. Every place
/// that picks a rule set (route rules, DNS rules, …) therefore sees it like any other entry.
///
/// Only **private domain / private IP** remain built-in; country lists (`geosite-cn`,
/// `geoip-cn`, `geolocation-!cn`) are user opt-in from the market, so we do not ship remote
/// resources most users neither need nor could remove. IDs are stable (persisted in
/// `local_override.json`) — never change them.
pub struct BuiltinRuleSetSpec {
    /// Stable entry ID (materialized as `CustomRuleSet.id`).
    pub id: &'static str,
    /// Display name (localized; user-editable).
    pub name: &'static str,
    /// Rule set tag referenced by route / DNS rules.
    pub tag: &'static str,
    /// Remote download URL (jsDelivr mirror of `meta-rules-dat@sing`).
    pub url: &'static str,
}

/// Built-in rule sets (order = materialized display order).
pub const BUILTIN_RULE_SETS: [BuiltinRuleSetSpec; 2] = [
    BuiltinRuleSetSpec {
        id: "builtin-ruleset-geosite-private",
        name: "私有域名",
        tag: "geosite-private",
        url: "https://testingcf.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@sing/geo/geosite/private.srs",
    },
    BuiltinRuleSetSpec {
        id: "builtin-ruleset-geoip-private",
        name: "私有 IP",
        tag: "geoip-private",
        url: "https://testingcf.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@sing/geo/geoip/private.srs",
    },
];

/// Retired built-in country/region rule set tags (for legacy-file cleanup; do not add more).
///
/// These tags no longer carry a built-in meaning — users who still need them add them from
/// the market (a custom entry with the same tag is fully equivalent). Legacy references in
/// `local_override.json` (rule-set reference entries plus the rules referencing them) are
/// cleaned up on load so no permanently unresolvable reference is left behind.
pub const RETIRED_BUILTIN_RULE_SET_TAGS: [&str; 3] = ["geosite-cn", "geoip-cn", "geolocation-!cn"];

/// CN-split remote rule-set registry (`tag`, jsDelivr URL), derived from
/// [`BUILTIN_RULE_SETS`] so the template registration and the materialized
/// custom entries never drift apart.
pub const CN_RULE_SETS: [(&str, &str); 2] = [
    (BUILTIN_RULE_SETS[0].tag, BUILTIN_RULE_SETS[0].url),
    (BUILTIN_RULE_SETS[1].tag, BUILTIN_RULE_SETS[1].url),
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

/// CN-split baseline DNS rules: `clash_mode direct → local`, `clash_mode global → proxy`;
/// `dns.final = remote` handles the rest.
///
/// The clash_mode rules make DNS follow the outbound mode switch (direct mode resolves
/// everything through the domestic resolver, global through the remote one) and deliberately
/// precede the FakeIP rule, so FakeIP never leaks into direct/global mode.
///
/// The built-in `geosite-cn → local` rule was dropped together with the country rule sets:
/// users who want domestic domains resolved locally add the `geosite-cn` rule set and a DNS
/// rule themselves (the built-in default must not reference a resource it does not ship).
pub fn cn_baseline_dns_rules() -> Vec<Value> {
    vec![
        json!({ "clash_mode": "direct", "action": "route", "server": "local" }),
        json!({ "clash_mode": "global", "action": "route", "server": "proxy" }),
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
    /// 成员是否模板动态计算（proxy/auto = 跟随订阅，不可编辑；global/final = 静态内置
    /// 成员集合，可在切片中编辑）。
    pub dynamic_members: bool,
    /// 静态成员（`dynamic_members = false` 时的模板与播种默认值）。
    pub members: &'static [&'static str],
    /// 默认成员 tag。
    pub default: &'static str,
}

/// See [`OUTBOUND_TAG_PROXY`].
pub const OUTBOUND_TAG_GLOBAL: &str = "global";
/// See [`OUTBOUND_TAG_PROXY`].
pub const OUTBOUND_TAG_FINAL: &str = "final";

/// 内置分组（顺序 = 列表展示顺序）。
///
/// `proxy` / `auto` 成员跟随订阅动态计算；`global`（Clash GLOBAL 语义：全局模式出站，
/// 含其它内置出站）/ `final`（兜底：节点选择 + 直连，`route.final` 指向它）成员为静态
/// 内置集合，可在切片中编辑。
pub const BUILTIN_OUTBOUND_GROUPS: [BuiltinOutboundGroupSpec; 4] = [
    BuiltinOutboundGroupSpec {
        id: "builtin-group-proxy",
        name: OUTBOUND_TAG_PROXY,
        selector: true,
        dynamic_members: true,
        members: &[],
        default: OUTBOUND_TAG_AUTO,
    },
    BuiltinOutboundGroupSpec {
        id: "builtin-group-auto",
        name: OUTBOUND_TAG_AUTO,
        selector: false,
        dynamic_members: true,
        members: &[],
        default: "",
    },
    BuiltinOutboundGroupSpec {
        id: "builtin-group-global",
        name: OUTBOUND_TAG_GLOBAL,
        selector: true,
        dynamic_members: false,
        members: &[
            OUTBOUND_TAG_PROXY,
            OUTBOUND_TAG_AUTO,
            OUTBOUND_TAG_FINAL,
            OUTBOUND_TAG_DIRECT,
            OUTBOUND_TAG_BLOCK,
        ],
        default: OUTBOUND_TAG_PROXY,
    },
    BuiltinOutboundGroupSpec {
        id: "builtin-group-final",
        name: OUTBOUND_TAG_FINAL,
        selector: true,
        dynamic_members: false,
        members: &[OUTBOUND_TAG_PROXY, OUTBOUND_TAG_DIRECT],
        default: OUTBOUND_TAG_PROXY,
    },
];

/// Built-in route rule spec: the source of truth materialized into the unified
/// `local_override` rule list (2026-09).
///
/// A built-in rule **is an ordinary rule**: once materialized the user may rename it, change
/// the match / action, reorder it or delete it, and restore it from this spec with the
/// "restore built-in rule" action. `enabled` / `action` / `sort_order` / `note` follow user
/// edits; retired built-in IDs are removed during the load migration. IDs are stable
/// (persisted in `local_override.json`) — never change them.
pub struct BuiltinRouteRuleSpec {
    /// Stable rule ID (materialized as `LocalRule.id`).
    pub id: &'static str,
    /// Display name (list title).
    pub name: &'static str,
    /// Comma-separated rule set tags (rendered as a sing-box `rule_set` array).
    pub target: &'static str,
    /// Default action: `true` = direct, `false` = main selector (proxy).
    pub direct: bool,
}

/// Built-in route rules (order = default materialized order: built-ins first, user rules after).
///
/// Only **private domain + private IP → direct** remains: LAN / loopback traffic must bypass
/// the proxy, the one split that actually breaks when missing. Everything else (CN / non-CN
/// split, …) is left to user-added rule sets and rules.
pub const BUILTIN_ROUTE_RULES: [BuiltinRouteRuleSpec; 1] = [BuiltinRouteRuleSpec {
    id: "builtin-private-direct",
    name: "内置：私有域名与私有 IP 直连",
    target: "geosite-private,geoip-private",
    direct: true,
}];

/// Retired built-in route rule IDs (for legacy-file cleanup; do not add more).
///
/// `builtin-cn-direct` (private + domestic direct) and `builtin-non-cn-proxy` (non-CN proxy)
/// were removed together with the country rule sets. Legacy entries flagged `builtin` that are
/// absent from the current spec are deleted on load, so no dead rule keeps referencing a
/// retired rule set.
pub const RETIRED_BUILTIN_ROUTE_RULE_IDS: [&str; 2] = ["builtin-cn-direct", "builtin-non-cn-proxy"];

/// CN-split baseline route rules: private destinations go `direct`; everything else falls
/// through to `route.final` (the main selector).
///
/// Since 2026-09 only the single private-direct rule remains. Built-in rules are materialized
/// into `local_override`, so this function only backs legacy callers (tests / previews).
///
/// `proxy_tag` is the main selector outbound tag; it is unused now that no baseline rule
/// targets the proxy group, but is kept so call sites keep working unchanged.
pub fn cn_baseline_route_rules(_proxy_tag: &str) -> Vec<Value> {
    vec![json!({
        "rule_set": ["geosite-private", "geoip-private"],
        "outbound": "direct"
    })]
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
