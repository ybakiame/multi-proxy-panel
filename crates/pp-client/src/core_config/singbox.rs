//! sing-box panel feature injection and Android DNS handling.

use serde_json::{Value, json};

use crate::config_slices::{DnsMode, DnsStrategy};

use super::PanelFeatures;

/// sing-box panel injection:
///
/// - `tun_enabled` → append to `inbounds` `{type: "tun", tag: "tun-in", address:
///   ["172.19.0.1/30", "fdfe:dcba:9876::1/126"], mtu: 9000, auto_route, stack}` (when template/override already has tun
///   inbound, replace it wholesale with settings; there can only be one `tun-in`);
///   Android build additionally aligns libbox/SFA paradigm by injecting `strict_route`
///   (see [`build_singbox_tun_inbound`]);
/// - `clash_api_enabled` → `experimental.clash_api = {external_controller:
///   "127.0.0.1:port", external_ui: "ui-<choice>", external_ui_download_url:
///   <by choice>, default_mode: <rule_mode>}`, append `secret` when non-empty (when template
///   already has `experimental.clash_api`, replace it wholesale); also injects baseline
///   `clash_mode` rules at the head of `route.rules` (see [`inject_mode_baseline_rules`]).
/// - `dns_fakeip_enabled` (opt-in) → fakeip DNS server + head DNS rules + `experimental.cache_file`
///   deep merge (see `apply_fakeip_mode` in `core_config::fakeip`); skipped on DNS takeover.
/// - always → TUN DNS hijack + domain sniff head rules (see [`inject_dns_hijack_and_sniff_rules`]).
///
/// `external_ui` directory name is distinguished by choice (`ui-yacd` / `ui-zashboard` /
/// `ui-metacubexd`), unknown falls back to zashboard:
/// sing-box only downloads panel zip when `external_ui` directory does not exist
/// (`external_ui_download_url`), fixed `ui` directory means switching choice only changes
/// download URL but the old panel in the directory never gets replaced — restart still shows
/// old panel. After directory distinction, new choice triggers re-download to new directory,
/// old directory残留 does not affect new panel. URL path remains `/ui`.
///
/// Dashboard open link does not need to change.
///
/// Note: sing-box `mode` is not composition-level semantics — it only matches the route rule
/// `clash_mode` condition (case-sensitive, see sing-box issue #2477). When Clash API is enabled,
/// the head of `route.rules` gets baseline `clash_mode` rules so direct/global actually take
/// effect, and `experimental.clash_api.default_mode` pins the startup mode to
/// `features.rule_mode` (small-case); the runtime push ([`push_clash_mode`]) is kept as a
/// secondary, idempotent path that also covers post-start mode switches.
pub fn apply_singbox_panel_features(composed: &mut Value, features: &PanelFeatures) {
    if features.tun_enabled {
        let Some(obj) = composed.as_object_mut() else {
            return;
        };
        let tun_inbound = build_singbox_tun_inbound(features, cfg!(target_os = "android"));
        let inbounds = obj
            .entry("inbounds")
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Some(arr) = inbounds.as_array_mut() {
            // Force override: remove template/override's own tun inbound, replace wholesale with settings.
            arr.retain(|inb| inb.get("type").and_then(|t| t.as_str()) != Some("tun"));
            arr.push(tun_inbound);
        }
    }

    if features.clash_api_enabled {
        let Some(obj) = composed.as_object_mut() else {
            return;
        };
        let mut clash_api = serde_json::Map::new();
        clash_api.insert(
            "external_controller".to_string(),
            Value::String(format!("127.0.0.1:{}", features.clash_api_port)),
        );
        // Panel UI: directory name distinguished by choice (`ui-<choice>`, unknown falls back
        // to zashboard) + download URL filled by choice. Directory distinction is the key to
        // switching taking effect: core only downloads panel zip when external_ui directory
        // does not exist, fixed `ui` directory means switching choice only changes download URL
        // but the directory already has old panel, never re-downloads (restart still shows old
        // panel); after directory distinction, new choice goes to new directory and re-downloads,
        // old directory残留 does not affect. URL path remains `/ui`.
        let ui_dir = format!(
            "ui-{}",
            super::normalized_clash_api_ui(&features.clash_api_ui)
        );
        clash_api.insert("external_ui".to_string(), Value::String(ui_dir));
        clash_api.insert(
            "external_ui_download_url".to_string(),
            Value::String(super::clash_api_ui_download_url(&features.clash_api_ui).to_string()),
        );
        // Startup mode pinned to the persisted (normalized small-case) rule mode: sing-box mode is
        // only the `clash_mode` rule matching value, core default is "Rule" (uppercase) — writing
        // default_mode removes the reliance on the post-start push succeeding (notably Android's
        // async VPN boot race where the push could fail and leave the mode at default).
        clash_api.insert(
            "default_mode".to_string(),
            Value::String(normalized_rule_mode(&features.rule_mode).to_string()),
        );
        if !features.clash_api_secret.is_empty() {
            clash_api.insert(
                "secret".to_string(),
                Value::String(features.clash_api_secret.clone()),
            );
        }
        // Force override: entire experimental.clash_api replaced by settings, preserving other
        // experimental fields.
        let experimental = obj
            .entry("experimental")
            .or_insert_with(|| Value::Object(Default::default()));
        if let Some(exp) = experimental.as_object_mut() {
            exp.insert("clash_api".to_string(), Value::Object(clash_api));
        }
        // Outbound mode baseline rules (Clash API enabled implies mode switching is possible).
        inject_mode_baseline_rules(obj);
    }

    // Android: after VpnService (TUN) takes over full traffic, system resolver is unavailable,
    // inject explicit DNS (remote goes through main outbound selector via DoH, local direct);
    // desktop relies on system resolver, not injected.
    //
    // ADR-0005 D1: `FollowSystem` (default) keeps the forced injection; `Takeover` skips it
    // because the config-slice DNS body was already applied at the ⓪ layer and the user takes
    // full responsibility. Desktop ignores `dns_mode` (injection is Android-only).
    #[cfg(target_os = "android")]
    if features.dns_mode != DnsMode::Takeover {
        inject_android_dns(composed);
    }

    // IPv6 switch (default off): rewrite the effective `dns.strategy` to `ipv4_only` so AAAA
    // records are not returned — a domain resolving to IPv6 through a node without an IPv6 egress
    // would otherwise fail to connect. Must run **after** `inject_android_dns` (which rewrites the
    // whole `dns` object with `strategy = prefer_ipv4`) so the override wins on Android too.
    //
    // `Takeover` exempt: the user's DNS slice already applied its own `strategy` at the ⓪ layer and
    // owns DNS completely (ADR-0005 D1), so the switch must not clobber it. `true` leaves the
    // template / injected `prefer_ipv4` untouched. Only writes when a `dns` object already exists.
    if !features.ipv6_enabled
        && features.dns_mode != DnsMode::Takeover
        && let Some(dns) = composed.get_mut("dns").and_then(Value::as_object_mut)
    {
        dns.insert(
            "strategy".to_string(),
            Value::String(DnsStrategy::Ipv4Only.as_str().to_string()),
        );
    }

    // FakeIP mode (opt-in, default off): injected last so it finalizes the DNS shape after
    // `inject_android_dns` and the IPv6 strategy override above. `Takeover` exempt: the user's
    // DNS slice already owns DNS completely at the ⓪ layer (ADR-0005 D1), so the whole fakeip
    // injection is skipped and the user's slice is left untouched.
    if features.dns_fakeip_enabled && features.dns_mode != DnsMode::Takeover {
        super::fakeip::apply_fakeip_mode(composed, features);
    }

    // TUN DNS 劫持 + 域名嗅探（无条件注入，见 inject_dns_hijack_and_sniff_rules）。
    if let Some(obj) = composed.as_object_mut() {
        inject_dns_hijack_and_sniff_rules(obj);
    }
}

/// Normalize rule mode for config injection: valid values `rule` / `global` / `direct` returned
/// as-is (small-case, matching [`push_clash_mode`] values and the mode-list sing-box registers
/// from the baseline `clash_mode` rules), anything else (including empty string) falls back to
/// `rule`. Callers already pass `normalized_rule_mode()` output, this keeps direct construction
/// (tests / preview) safe as well.
fn normalized_rule_mode(rule_mode: &str) -> &str {
    match rule_mode {
        "rule" | "global" | "direct" => rule_mode,
        _ => "rule",
    }
}

/// Outbound mode baseline rules (injected at the head of `route.rules` when Clash API is enabled).
///
/// sing-box `mode` is not core built-in semantics — it is only the value matched by the route
/// rule `clash_mode` condition (case-sensitive, see sing-box issue #2477). Without any
/// `clash_mode` rule the core does not even register `rule`/`direct`/`global` in its mode list,
/// and Clash API `PATCH /configs {"mode": ...}` silently has no effect. The two head rules make
/// the mode switch real:
///
/// - `{ "clash_mode": "direct", "outbound": "direct" }`: direct mode → all traffic direct;
/// - `{ "clash_mode": "global", "outbound": "proxy" }`: global mode → all traffic through the
///   template's main selector group (`proxy`).
///
/// rule mode needs no baseline rule — traffic falls through to the normal rule chain below.
///
/// Priority semantics: mode switch > local override rules > MITM whitelist (desktop) >
/// subscription/template rules > final. `insert(0)` guarantees the mode switch wins over every
/// later rule (including the MITM whitelist rule `compose_singbox_config` prepends and the local
/// override rules `apply_singbox_local_override` prepends — both run before this stage).
///
/// Idempotency guard: when `route.rules` already contains any rule carrying a `clash_mode`
/// condition (user explicitly took over mode semantics via Profile override / template), the
/// injection is skipped (debug log) to avoid duplicating or conflicting with user rules.
fn inject_mode_baseline_rules(obj: &mut serde_json::Map<String, Value>) {
    let route = obj
        .entry("route")
        .or_insert_with(|| Value::Object(Default::default()));
    let Some(route_obj) = route.as_object_mut() else {
        return;
    };
    let rules = route_obj
        .entry("rules")
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(rules_arr) = rules.as_array_mut() else {
        return;
    };
    if rules_arr.iter().any(|r| r.get("clash_mode").is_some()) {
        tracing::debug!(
            "route.rules 已含 clash_mode 条件规则，跳过基础模式规则注入（用户经 Profile 覆写显式接管模式语义）"
        );
        return;
    }
    let baseline = [
        json!({ "clash_mode": "direct", "outbound": "direct" }),
        json!({ "clash_mode": "global", "outbound": "proxy" }),
    ];
    for (i, rule) in baseline.into_iter().enumerate() {
        rules_arr.insert(i, rule);
    }
}

/// TUN DNS 劫持 + 域名嗅探头部规则（无条件注入，见 [`apply_singbox_panel_features`]）。
///
/// sing-box 1.13+ 移除 inbound 级 `sniff` 字段（见 [`build_singbox_tun_inbound`] 注释），TUN
/// (Android VPN / desktop tun) 模式下域名分流规则与 `dns.servers` 需要嗅探 + DNS 劫持才生效。
/// 标准做法是在 `route.rules` 头部注入两条 action 规则：
///
/// - `{ "action": "sniff" }`：对所有连接做协议/域名嗅探（此后续规则可用嗅探出的域名分流）；
/// - `{ "protocol": "dns", "action": "hijack-dns" }`：DNS 流量劫持进 DNS 模块（放 clash_mode
///   之前：direct/global 模式下 DNS 也必须进 DNS 模块解析，否则 TUN 下 system resolver 不可用，
///   `dns.servers` 形同虚设）。
///
/// 无条件注入：TUN 场景必需；mixed-only（无 TUN）场景下嗅探无副作用、DNS 劫持不命中，无害。
/// 与 clash_mode 注入解耦（用户经 Profile 覆写显式接管 mode 语义时仍注入 sniff/hijack-dns）。
///
/// 幂等性：已有 `{"action":"sniff"}` 规则时跳过 sniff；已有 protocol=dns 的 hijack-dns 规则时
/// 跳过 hijack-dns（各自独立判断，避免与用户/模板显式规则重复）。
fn inject_dns_hijack_and_sniff_rules(obj: &mut serde_json::Map<String, Value>) {
    let route = obj
        .entry("route")
        .or_insert_with(|| Value::Object(Default::default()));
    let Some(route_obj) = route.as_object_mut() else {
        return;
    };
    let rules = route_obj
        .entry("rules")
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(rules_arr) = rules.as_array_mut() else {
        return;
    };
    let has_sniff = rules_arr
        .iter()
        .any(|r| r.get("action").and_then(|a| a.as_str()) == Some("sniff"));
    let has_hijack_dns = rules_arr.iter().any(|r| {
        r.get("action").and_then(|a| a.as_str()) == Some("hijack-dns")
            && r.get("protocol").and_then(|p| p.as_str()) == Some("dns")
    });
    if !has_sniff {
        rules_arr.insert(0, json!({ "action": "sniff" }));
    }
    if !has_hijack_dns {
        // hijack-dns 紧随 sniff 之后（index 0 = sniff 时插到 1，否则插到头部 0）。
        let at = usize::from(
            rules_arr
                .first()
                .is_some_and(|r| r.get("action").and_then(|a| a.as_str()) == Some("sniff")),
        );
        rules_arr.insert(at, json!({ "protocol": "dns", "action": "hijack-dns" }));
    }
}

/// Build sing-box tun inbound JSON (libbox-compatible field set).
///
/// Base fields (both platforms): `type = tun`, `tag = tun-in`, `address =
/// ["172.19.0.1/30", "fdfe:dcba:9876::1/126"]`, `mtu = 9000`, `auto_route`, `stack`.
///
/// The address is dual-stack (IPv4 + IPv6, values matching the official sing-box client example):
/// DNS strategy `prefer_ipv4` still returns AAAA records for dual-stack domains, so an IPv4-only
/// TUN would let the app dial IPv6 into a route-less tunnel — packets never enter the TUN and get
/// blackholed / leak for blocked domains. Carrying an IPv6 address keeps that traffic inside the
/// TUN (proxied or direct per routing rules) instead of leaking or timing out.
///
/// Android (libbox / VpnService takes over traffic) additionally aligns sing-box for Android
/// paradigm by injecting `strict_route = true` — libbox only calls back `openTun()` to establish
/// VPN interface when config contains tun inbound, field set must be within its compatibility
/// range; `interface_name` / `fd` and other desktop-specific fields are not injected (libbox
/// resolves interface name via `getTunnelName(fd)` itself, these fields cause problems on Android).
/// Desktop keeps original field set to avoid changing desktop core behavior.
///
/// Note: sing-box 1.13.0+ removes inbound-level `sniff` legacy field (`check -c` directly rejects),
/// SFA old config's `sniff: true` is no longer compatible, so not injected; domain sniffing uses
/// route rule action `{"action": "sniff"}` (see routing config).
pub fn build_singbox_tun_inbound(features: &PanelFeatures, is_android: bool) -> Value {
    let mut tun = serde_json::Map::new();
    tun.insert("type".to_string(), json!("tun"));
    tun.insert("tag".to_string(), json!("tun-in"));
    tun.insert(
        "address".to_string(),
        json!(["172.19.0.1/30", "fdfe:dcba:9876::1/126"]),
    );
    tun.insert("mtu".to_string(), json!(9000));
    tun.insert("auto_route".to_string(), json!(features.tun_auto_route));
    tun.insert("stack".to_string(), json!(features.tun_stack));
    if is_android {
        tun.insert("strict_route".to_string(), json!(true));
    }
    Value::Object(tun)
}

/// Read the main outbound selector tag from composed config (Android DNS `remote` server detour target).
///
/// Priority: first `type = selector` outbound tag (`singbox_template` fixed generates `proxy`,
/// subscription self-built groups are also mostly selector groups); when no selector, fall back
/// to `route.final` (still an outbound tag); when still cannot determine, return `None`
/// (caller skips DNS injection to avoid injecting illegal detour).
pub(crate) fn main_outbound_selector_tag(obj: &serde_json::Map<String, Value>) -> Option<String> {
    if let Some(outbounds) = obj.get("outbounds").and_then(Value::as_array) {
        for outbound in outbounds {
            if outbound.get("type").and_then(Value::as_str) == Some("selector")
                && let Some(tag) = outbound.get("tag").and_then(Value::as_str)
            {
                return Some(tag.to_string());
            }
        }
    }
    obj.get("route")
        .and_then(|r| r.get("final"))
        .and_then(Value::as_str)
        .map(String::from)
}

/// Android explicit DNS injection (only Android: called by [`apply_singbox_panel_features`] when
/// `cfg!(target_os = "android")`; not injected on local/desktop).
///
/// Android by VpnService (TUN) takes over full traffic, system resolver is unavailable (DNS queries
/// will be tun-looped or leaked), must explicitly declare DNS servers and specify detour outbound.
/// Injected `dns` section:
///
/// - `remote`: DoH (1.1.1.1), through main outbound selector ( `detour` reads actual selector tag
///   from composed config, see [`main_outbound_selector_tag`], not hardcoded); when detour target
///   is "empty direct outbound", omit `detour` field (see [`is_empty_direct_outbound`]);
/// - `local`: UDP (223.5.5.5), no `detour` field — omit means default direct dial,
///   semantically equivalent and always legal;
/// - `rules` empty array, `final = remote`, `reverse_mapping = true`, `strategy = prefer_ipv4`.
///   `reverse_mapping` lets sing-box map a hijacked-DNS-resolved IP back to its domain so the
///   Clash API `metadata.host` is populated for TUN connections whose payload cannot be sniffed
///   (otherwise the connection record domain stays blank).
///
/// Servers use new format (`type` + `server`): libbox (sing-box 1.12) natively parses,
/// real `sing-box check` (1.13+, legacy `address` format needs
/// `ENABLE_DEPRECATED_LEGACY_DNS_SERVERS`) passes without environment variable. After injection
/// supplement `route.default_domain_resolver` (sing-box 1.12+ requires explicit declaration when
/// `dns.servers` exists, otherwise `check` rejects, see [`ensure_domain_resolver`]).
///
/// Note: sing-box rejects DNS server detour to "empty direct outbound" at startup phase (error
/// `detour to an empty direct outbound makes no sense`, empty = DialerOptions all default).
/// This restriction applies to any empty direct outbound (including explicitly declared), not just
/// built-in direct; direct outbound with extra config keys (e.g., `override_address`) can be used
/// as detour target.
///
/// After DNS section injection, supplement `domain_resolver = {"server": "local"}` for each
/// outbound containing `server` field (see [`ensure_outbound_domain_resolvers`]). sing-box 1.12+
/// outbound dialer resolves `server` domain (e.g., proxy server domain `proxy-panel.ybakiame.net`)
/// by falling back to `route.default_domain_resolver` — which is the injected remote (DoH through
/// main proxy outbound), and remote needs to connect to proxy first → "resolve proxy server domain
/// through proxy" loop. Explicit `domain_resolver = {"server": "local"}` makes proxy server domain
/// go through local UDP direct resolution, direct dial, avoiding loop (aligns with husi
/// `ConfigBuilder.kt` reference approach).
pub fn inject_android_dns(composed: &mut Value) {
    let Some(obj) = composed.as_object_mut() else {
        return;
    };
    let Some(detour) = main_outbound_selector_tag(obj) else {
        return;
    };
    // remote server conditionally includes detour: omit when target is empty direct outbound
    // (sing-box rejects at startup phase).
    let mut remote = serde_json::Map::new();
    remote.insert("tag".to_string(), json!("remote"));
    remote.insert("type".to_string(), json!("https"));
    remote.insert("server".to_string(), json!("1.1.1.1"));
    remote.insert("server_port".to_string(), json!(443));
    if !is_empty_direct_outbound(obj, &detour) {
        remote.insert("detour".to_string(), json!(detour));
    }
    obj.insert(
        "dns".to_string(),
        json!({
            "servers": [
                Value::Object(remote),
                { "tag": "local", "type": "udp", "server": "223.5.5.5", "server_port": 53 }
            ],
            "rules": [],
            "final": "remote",
            "reverse_mapping": true,
            "strategy": "prefer_ipv4"
        }),
    );
    super::ensure_domain_resolver(obj);
    // Proxy outbound server domain resolved via local direct, avoiding remote loop (see function docs).
    ensure_outbound_domain_resolvers(obj);
}

/// Whether the outbound pointed to by `tag` is an "empty direct outbound" (DialerOptions all default,
/// only contains `type` / `tag` two keys).
///
/// sing-box rejects DNS server detour to empty direct outbound at startup phase (error
/// `detour to an empty direct outbound makes no sense`) — this restriction applies to any empty
/// direct outbound (including explicitly declared), not just built-in; direct outbound with extra
/// config keys can be used as detour target. Therefore remote DNS server detour target being
/// "empty direct outbound" must be omitted.
///
/// Returns `false` when: outbound not found, not `direct` type, or direct has extra config keys.
pub(crate) fn is_empty_direct_outbound(obj: &serde_json::Map<String, Value>, tag: &str) -> bool {
    let Some(outbounds) = obj.get("outbounds").and_then(Value::as_array) else {
        return false;
    };
    for outbound in outbounds {
        let Some(outbound_obj) = outbound.as_object() else {
            continue;
        };
        if outbound_obj.get("tag").and_then(Value::as_str) != Some(tag) {
            continue;
        }
        let is_direct = outbound_obj.get("type").and_then(Value::as_str) == Some("direct");
        return is_direct
            && outbound_obj
                .keys()
                .all(|k| matches!(k.as_str(), "type" | "tag"));
    }
    false
}

/// Inject `domain_resolver` for outbounds containing `server` field, pointing to direct DNS server `local`.
///
/// sing-box 1.12+ outbound dialer resolves `server` domain (e.g., proxy server domain) by falling back
/// to `route.default_domain_resolver`; Android injected default resolver is remote (DoH through main
/// proxy outbound), causing proxy server domain "resolved through proxy" → DoH needs to connect to
/// proxy first → loop. Explicit `domain_resolver = {"server": "local"}` makes proxy server domain
/// go through local UDP direct resolution (local has no detour = direct dial), avoiding loop
/// (aligns with husi `ConfigBuilder.kt` reference approach).
///
/// Only processes outbounds containing string `server` field (selector/urltest/direct/block etc.
/// without `server` field are not touched); outbounds already having `domain_resolver` are not
/// overridden (respect subscription/template explicit config).
pub(crate) fn ensure_outbound_domain_resolvers(obj: &mut serde_json::Map<String, Value>) {
    let Some(outbounds) = obj.get_mut("outbounds").and_then(Value::as_array_mut) else {
        return;
    };
    for outbound in outbounds {
        let Some(outbound_obj) = outbound.as_object_mut() else {
            continue;
        };
        if outbound_obj.get("server").and_then(Value::as_str).is_none() {
            continue;
        }
        if outbound_obj.contains_key("domain_resolver") {
            continue;
        }
        outbound_obj.insert("domain_resolver".to_string(), json!({ "server": "local" }));
    }
}
