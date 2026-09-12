//! 内置基线视图（read-only）：把 [`super::baseline`] 的 CN 分流基线派生成前端可展示的
//! 只读结构。
//!
//! 用途仅限展示：规则集 / 路由规则 / DNS 规则 / 内置出站都是核心模板内置项，前端不可编辑、
//! 不可删除。视图不复制真值——规则集 tag/URL 直接取自 [`CN_RULE_SETS`]，路由 / DNS 规则由
//! [`cn_baseline_route_rules`] / [`cn_baseline_dns_rules`] 的实际输出派生，出站 tag/kind 引用
//! [`super::baseline`] 的常量，避免与注入逻辑（[`crate::profile::singbox_template`]）产生双
//! 真相源漂移。
//!
//! FakeIP 属可选模式（`PanelFeatures::dns_fakeip_enabled`，见 [`super::fakeip`]），开启时才
//! 追加 fakeip server / DNS 规则，本视图保持**静态基线**，不反映该条件性内容。

use serde::Serialize;
use serde_json::Value;

use super::baseline::{
    CN_RULE_SETS, OUTBOUND_KIND_BLOCK, OUTBOUND_KIND_DIRECT, OUTBOUND_KIND_SELECTOR,
    OUTBOUND_KIND_URLTEST, OUTBOUND_TAG_AUTO, OUTBOUND_TAG_BLOCK, OUTBOUND_TAG_DIRECT,
    OUTBOUND_TAG_PROXY, cn_baseline_dns_rules, cn_baseline_route_rules,
};

/// 内置规则集视图项（只读）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BaselineRuleSetView {
    /// 规则集 tag（与 [`CN_RULE_SETS`] 一致）。
    pub tag: String,
    /// 远程规则集 URL（与 [`CN_RULE_SETS`] 一致）。
    pub url: String,
    /// 中文友好名（如「国内域名」）。
    pub description: String,
}

/// 内置路由规则视图项（只读）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BaselineRuleView {
    /// 中文描述（如「国内域名 → 直连」）。
    pub description: String,
    /// 命中的规则集 tag 列表。
    pub rule_set_tags: Vec<String>,
    /// 命中后的出站 tag。
    pub outbound: String,
}

/// 内置 DNS 规则视图项（只读）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BaselineDnsRuleView {
    /// 中文描述（如「直连模式：DNS 走本地解析器」）。
    pub description: String,
    /// 分流到的 DNS server tag（`local` / `remote`）。
    pub server: String,
    /// clash_mode 条件（`direct` / `global`）；非模式规则为 `None` 且不序列化。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clash_mode: Option<String>,
    /// 命中的规则集 tag 列表；模式规则为空且不序列化。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rule_set_tags: Vec<String>,
}

/// 内置出站视图项（只读）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BaselineOutboundView {
    /// 出站 tag（引用 [`super::baseline`] 常量）。
    pub tag: String,
    /// 出站 kind（sing-box `type`，引用 [`super::baseline`] 常量）。
    pub kind: String,
    /// 中文描述（如「手动选择」）。
    pub description: String,
}

/// 内置 CN 分流基线只读视图。
///
/// 前端把本结构与各配置列表只读合并展示；所有项均不可编辑 / 删除。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BaselineView {
    /// 5 个 CN 规则集。
    pub rule_sets: Vec<BaselineRuleSetView>,
    /// 5 条基线路由规则。
    pub route_rules: Vec<BaselineRuleView>,
    /// 基线 DNS 规则摘要（clash_mode 分流 + geosite-cn → local）。
    pub dns_rules: Vec<BaselineDnsRuleView>,
    /// 内置出站：proxy / auto / direct / block。
    pub outbounds: Vec<BaselineOutboundView>,
    /// `route.final`（主选择器 tag）。
    pub route_final: String,
}

impl BaselineView {
    /// 从基线真值源派生只读视图。
    ///
    /// 纯静态、无 IO、无参数：规则集 / 路由规则 / DNS 规则分别取自 [`CN_RULE_SETS`]、
    /// [`cn_baseline_route_rules`]、[`cn_baseline_dns_rules`]，出站 tag/kind 引用常量。
    #[must_use]
    pub fn new() -> Self {
        let rule_sets = CN_RULE_SETS
            .iter()
            .map(|(tag, url)| BaselineRuleSetView {
                tag: (*tag).to_string(),
                url: (*url).to_string(),
                description: rule_set_description(tag).to_string(),
            })
            .collect();

        let route_rules = cn_baseline_route_rules(OUTBOUND_TAG_PROXY)
            .iter()
            .map(|rule| {
                let rule_set_tags = string_array(rule.get("rule_set"));
                let outbound = rule
                    .get("outbound")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let names = rule_set_tags
                    .iter()
                    .map(|tag| rule_set_description(tag))
                    .collect::<Vec<_>>()
                    .join("、");
                BaselineRuleView {
                    description: format!("{names} → {}", route_outbound_description(&outbound)),
                    rule_set_tags,
                    outbound,
                }
            })
            .collect();

        let dns_rules = cn_baseline_dns_rules()
            .iter()
            .map(|rule| {
                let clash_mode = rule
                    .get("clash_mode")
                    .and_then(Value::as_str)
                    .map(String::from);
                let rule_set_tags = string_array(rule.get("rule_set"));
                let server = rule
                    .get("server")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let subject = match &clash_mode {
                    Some(mode) => format!("{}模式", clash_mode_label(mode)),
                    None => rule_set_tags
                        .iter()
                        .map(|tag| rule_set_description(tag))
                        .collect::<Vec<_>>()
                        .join("、"),
                };
                BaselineDnsRuleView {
                    description: format!("{subject}：DNS 走{}解析器", dns_server_label(&server)),
                    server,
                    clash_mode,
                    rule_set_tags,
                }
            })
            .collect();

        let outbounds = vec![
            BaselineOutboundView {
                tag: OUTBOUND_TAG_PROXY.to_string(),
                kind: OUTBOUND_KIND_SELECTOR.to_string(),
                description: outbound_description(OUTBOUND_TAG_PROXY).to_string(),
            },
            BaselineOutboundView {
                tag: OUTBOUND_TAG_AUTO.to_string(),
                kind: OUTBOUND_KIND_URLTEST.to_string(),
                description: outbound_description(OUTBOUND_TAG_AUTO).to_string(),
            },
            BaselineOutboundView {
                tag: OUTBOUND_TAG_DIRECT.to_string(),
                kind: OUTBOUND_KIND_DIRECT.to_string(),
                description: outbound_description(OUTBOUND_TAG_DIRECT).to_string(),
            },
            BaselineOutboundView {
                tag: OUTBOUND_TAG_BLOCK.to_string(),
                kind: OUTBOUND_KIND_BLOCK.to_string(),
                description: outbound_description(OUTBOUND_TAG_BLOCK).to_string(),
            },
        ];

        Self {
            rule_sets,
            route_rules,
            dns_rules,
            outbounds,
            route_final: OUTBOUND_TAG_PROXY.to_string(),
        }
    }
}

impl Default for BaselineView {
    fn default() -> Self {
        Self::new()
    }
}

/// 规则集 tag → 中文友好名。
fn rule_set_description(tag: &str) -> &'static str {
    if tag == "geosite-cn" {
        "国内域名"
    } else if tag == "geoip-cn" {
        "国内 IP"
    } else if tag == "geolocation-!cn" {
        "非中国大陆域名"
    } else if tag == "geosite-private" {
        "私有域名"
    } else if tag == "geoip-private" {
        "私有 IP"
    } else {
        "自定义规则集"
    }
}

/// 内置出站 tag → 中文描述（出站列表用）。
fn outbound_description(tag: &str) -> &'static str {
    if tag == OUTBOUND_TAG_PROXY {
        "手动选择"
    } else if tag == OUTBOUND_TAG_AUTO {
        "自动测速"
    } else if tag == OUTBOUND_TAG_DIRECT {
        "直连"
    } else if tag == OUTBOUND_TAG_BLOCK {
        "拦截"
    } else {
        "代理"
    }
}

/// 路由规则出站 tag → 中文动作描述（「→ 直连」用）。
fn route_outbound_description(tag: &str) -> &'static str {
    if tag == OUTBOUND_TAG_DIRECT {
        "直连"
    } else if tag == OUTBOUND_TAG_BLOCK {
        "拦截"
    } else if tag == OUTBOUND_TAG_PROXY {
        "代理"
    } else if tag == OUTBOUND_TAG_AUTO {
        "自动测速"
    } else {
        "代理"
    }
}

/// clash_mode → 中文标签。
fn clash_mode_label(mode: &str) -> &'static str {
    match mode {
        "direct" => "直连",
        "global" => "全局",
        _ => "规则",
    }
}

/// DNS server tag → 中文标签。
fn dns_server_label(server: &str) -> &'static str {
    match server {
        "local" => "本地",
        "remote" => "远程",
        "fakeip" => "FakeIP",
        _ => "默认",
    }
}

/// 把 `rule_set` 匹配字段（单字符串或数组）规整为 tag 列表。
fn string_array(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        _ => Vec::new(),
    }
}
