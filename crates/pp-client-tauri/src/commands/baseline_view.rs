//! 内置基线视图命令（只读展示用途）。
//!
//! 前端用 [`baseline_view_get`] 拉取内置 CN 分流基线（规则集 / 路由规则 / DNS 规则 / 内置
//! 出站 / `route.final`）并入各配置列表只读展示。所有项均为核心模板内置，**不可编辑 / 不可
//! 删除**；本命令无参、纯静态、不读盘，仅把 [`pp_client::core_config::BaselineView`] 原样
//! 序列化返回。

use pp_client::core_config::BaselineView;

// ---------------------------------------------------------------------------
// Pure command bodies (unit-testable without a Tauri runtime)
// ---------------------------------------------------------------------------

/// 构造内置基线只读视图（无 IO、无参数）。
pub(crate) fn baseline_view() -> BaselineView {
    BaselineView::new()
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// 获取内置 CN 分流基线只读视图。
#[tauri::command]
pub fn baseline_view_get() -> Result<BaselineView, String> {
    Ok(baseline_view())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pp_client::core_config::CN_RULE_SETS;

    /// 视图规则集与基线真值源 [`CN_RULE_SETS`] 一一对应（tag / URL 不漂移），
    /// 且路由 / 出站数量与 `route_final` 符合预期。
    #[test]
    fn view_rule_sets_match_baseline_source() {
        let view = baseline_view();

        assert_eq!(view.rule_sets.len(), CN_RULE_SETS.len());
        for (item, (tag, url)) in view.rule_sets.iter().zip(CN_RULE_SETS) {
            assert_eq!(item.tag, tag);
            assert_eq!(item.url, url);
            assert!(!item.description.is_empty(), "description 不应为空");
        }

        assert_eq!(view.route_rules.len(), 5);
        assert_eq!(view.dns_rules.len(), 3);
        assert_eq!(view.outbounds.len(), 4);
        assert_eq!(view.route_final, "proxy");
    }

    /// 序列化形态断言：前端消费的字段名 / 类型固定。
    #[test]
    fn view_serializes_expected_shape() {
        let value = serde_json::to_value(baseline_view()).unwrap();

        assert_eq!(value["rule_sets"][0]["tag"], "geosite-private");
        assert_eq!(value["route_rules"][0]["outbound"], "direct");
        assert!(value["route_rules"][0]["rule_set_tags"].is_array());
        assert_eq!(value["dns_rules"][0]["clash_mode"], "direct");
        assert_eq!(value["dns_rules"][0]["server"], "local");
        assert_eq!(value["outbounds"][0]["tag"], "proxy");
        assert_eq!(value["outbounds"][0]["kind"], "selector");
        assert_eq!(value["route_final"], "proxy");

        // 非模式 DNS 规则不带 clash_mode 字段（serde skip）。
        assert!(value["dns_rules"][2].get("clash_mode").is_none());
        assert_eq!(value["dns_rules"][2]["rule_set_tags"][0], "geosite-cn");
    }
}
