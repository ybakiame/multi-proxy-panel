//! Android explicit DNS injection (VpnService takes over the system resolver).
//!
//! Split out of `super::singbox` to keep each module under the file-size gate; see
//! [`inject_android_dns`] for the injection contract.

use serde_json::{Value, json};

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
/// Injected `dns` section (same CN-split shape as [`crate::profile::singbox_template`]):
///
/// - `local`: DoH (`https` 223.5.5.5:443), no `detour` field — omit means default direct dial,
///   semantically equivalent and always legal;
/// - `remote`: DoH (`https` 8.8.8.8:443), through main outbound selector (`detour` reads actual
///   selector tag from composed config, see [`main_outbound_selector_tag`], not hardcoded); DoH
///   on 443 is used instead of DoT on 853 because it shares the well-worn HTTPS path through the
///   proxy (some node egress networks interfere with bare 853), aligning with the reference
///   templates (`platforms/android/realip.json` / `fakeip.json` both use `type: https` for the
///   proxied resolver); when detour target is "empty direct outbound", omit `detour` field (see
///   [`is_empty_direct_outbound`]);
/// - `rules` = [`super::cn_baseline_dns_rules`] (`clash_mode` direct/global → local/remote,
///   `geosite-cn` → local), `final = remote`, `reverse_mapping = true`, `strategy = prefer_ipv4`.
///   `reverse_mapping` lets sing-box map a hijacked-DNS-resolved IP back to its domain so the
///   Clash API `metadata.host` is populated for TUN connections whose payload cannot be sniffed
///   (otherwise the connection record domain stays blank).
///
/// Servers use new format (`type` + `server`): libbox (sing-box 1.12) natively parses,
/// real `sing-box check` (1.13+, legacy `address` format needs
/// `ENABLE_DEPRECATED_LEGACY_DNS_SERVERS`) passes without environment variable. After injection
/// supplement `route.default_domain_resolver` (sing-box 1.12+ requires explicit declaration when
/// `dns.servers` exists, otherwise `check` rejects, see [`ensure_domain_resolver`]) and the
/// CN-split rule-set registry (the injected `geosite-cn` DNS rule must resolve, see
/// [`super::ensure_cn_rule_sets`]).
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
/// by falling back to `route.default_domain_resolver` — which is `local` (the first tagged
/// server), and local needs a direct dial, avoiding the "resolve proxy server domain through
/// proxy" loop. Explicit `domain_resolver = {"server": "local"}` makes proxy server domain
/// go through local direct resolution, direct dial, avoiding loop (aligns with husi
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
    remote.insert("server".to_string(), json!("8.8.8.8"));
    remote.insert("server_port".to_string(), json!(443));
    if !is_empty_direct_outbound(obj, &detour) {
        remote.insert("detour".to_string(), json!(detour));
    }
    obj.insert(
        "dns".to_string(),
        json!({
            "servers": [
                { "tag": "local", "type": "https", "server": "223.5.5.5", "server_port": 443 },
                Value::Object(remote)
            ],
            "rules": super::cn_baseline_dns_rules(),
            "final": "remote",
            "reverse_mapping": true,
            "strategy": "prefer_ipv4"
        }),
    );
    // The injected `geosite-cn` DNS rule references a rule set; make sure the CN-split registry
    // exists even when called on a config that did not come from `singbox_template` (idempotent).
    let route = obj
        .entry("route")
        .or_insert_with(|| Value::Object(Default::default()));
    if let Some(route_obj) = route.as_object_mut() {
        super::ensure_cn_rule_sets(route_obj);
    }
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
