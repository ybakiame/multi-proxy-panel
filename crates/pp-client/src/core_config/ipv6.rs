//! IPv6-off route-layer fast fail for sing-box.
//!
//! See [`inject_ipv6_reject_rule`] for the full three-layer v6 defense rationale.

use serde_json::{Value, json};

/// IPv6-off route-layer fast fail: inject `{"ip_version": 6, "action": "reject"}` into
/// `route.rules` right after the `hijack-dns` rule.
///
/// # Why a route rule (the third v6 defense layer)
///
/// The IPv6 switch is enforced at three layers, each covering a different bypass:
///
/// 1. **Dual-stack TUN** ([`super::singbox::build_singbox_tun_inbound`]): the tun inbound carries
///    an IPv6 address so v6 packets enter the tunnel instead of leaking or being silently
///    blackholed outside it;
/// 2. **DNS strategy** (`dns.strategy = ipv4_only`, the override in
///    [`super::singbox::apply_singbox_panel_features`]): suppresses AAAA for DNS queries that are
///    hijacked into the sing-box DNS module;
/// 3. **Route reject** (this function): apps that resolve names through their own HTTPDNS / DoH
///    (Chrome Secure DNS, in-app resolvers, …) never ask the hijacked resolver, so layer 2 never
///    sees their query and they dial a real IPv6 address directly. Inside the tunnel that
///    connection either hangs (direct target with no local v6) or aborts (proxy node with no v6
///    egress). Rejecting it deterministically fails fast so the app immediately falls back to
///    IPv4 instead of hanging (the reference `fakeip.json` template does the same).
///
/// # Position
///
/// Inserted immediately after `hijack-dns` and therefore before the `clash_mode` baseline and
/// every subscription / user / CN-split rule. `sniff` precedes it (the rule matches the resolved
/// destination IP, not a domain); `hijack-dns` precedes it so DNS queries are intercepted by the
/// DNS module rather than rejected as IPv6 traffic. The realip-mode `resolve` action rule is
/// injected one stage later (see `super::singbox::inject_resolve_rule`) and lands between
/// `hijack-dns` and this reject, matching the reference template order
/// `sniff → hijack-dns → resolve → v6 reject`.
///
/// # No DNS-takeover exemption
///
/// Unlike the DNS strategy override and the fakeip injection, this rule is **not** skipped when
/// `dns_mode` is [`crate::config_slices::DnsMode::Takeover`]: it operates at the routing layer,
/// not the DNS layer, and is independent of who owns DNS. Even a user who fully owns DNS still
/// routes v6 connections through the panel's dual-stack TUN; with the v6 switch off those
/// connections must fail fast.
///
/// # Idempotency
///
/// When `route.rules` already carries an `ip_version = 6` rule with `action` `reject` or
/// `resolve`, the injection is skipped (debug log): the user explicitly took over IPv6 handling
/// via Profile override / template, and prepending a reject would either duplicate their rule or
/// shadow a `resolve` they asked for.
pub(super) fn inject_ipv6_reject_rule(obj: &mut serde_json::Map<String, Value>) {
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
    let has_user_override = rules_arr.iter().any(|r| {
        r.get("ip_version").and_then(Value::as_i64) == Some(6)
            && matches!(
                r.get("action").and_then(Value::as_str),
                Some("reject" | "resolve")
            )
    });
    if has_user_override {
        tracing::debug!(
            "route.rules 已含 ip_version=6 的 reject/resolve 规则，跳过 IPv6 快速拒绝规则注入（用户覆写）"
        );
        return;
    }
    // 紧随 hijack-dns 之后；sniff/hijack-dns 由 `inject_dns_hijack_and_sniff_rules` 无条件
    // 先行注入，正常情况下必能命中。找不到时退回头部（不阻断功能）。
    let at = rules_arr
        .iter()
        .position(|r| {
            r.get("action").and_then(Value::as_str) == Some("hijack-dns")
                && r.get("protocol").and_then(Value::as_str) == Some("dns")
        })
        .map_or(0, |i| i + 1);
    rules_arr.insert(at, json!({ "ip_version": 6, "action": "reject" }));
}
