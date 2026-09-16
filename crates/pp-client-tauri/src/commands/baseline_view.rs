//! 内置基线视图命令（只读展示用途）。
//!
//! 前端用 [`baseline_view_get`] 拉取内置 CN 分流基线（规则集 / 路由规则 / DNS 规则 / 内置
//! 出站 / `route.final`）并入各配置列表只读展示。所有项均为核心模板内置，**不可编辑 / 不可
//! 删除**；本命令无参、纯静态、不读盘，仅把 [`pp_client::core_config::BaselineView`] 原样
//! 序列化返回。

use pp_client::ClientConfig;
use pp_client::config_slices::DnsSlice;
use pp_client::core_config::BaselineView;
use tauri::State;

use crate::state::AppState;

// ---------------------------------------------------------------------------
// Pure command bodies (unit-testable without a Tauri runtime)
// ---------------------------------------------------------------------------

/// 构造内置基线只读视图（无 IO、无参数）。
pub(crate) fn baseline_view() -> BaselineView {
    BaselineView::new()
}

/// 构造内置默认 DNS 的可编辑切片视图（无 IO；`ipv6_enabled` 决定丢弃规则是否含 AAAA 与
/// 默认 strategy）。
pub(crate) fn builtin_dns_slice(ipv6_enabled: bool) -> DnsSlice {
    pp_client::core_config::builtin_dns_slice(ipv6_enabled)
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// 获取内置 CN 分流基线只读视图。
#[tauri::command]
pub fn baseline_view_get() -> Result<BaselineView, String> {
    Ok(baseline_view())
}

/// 获取内置默认 DNS 配置（切片 schema 形态，可编辑）。
///
/// 前端 DNS 可视化页用它初始化「跟随系统」模式的草稿：内置默认直接映射进编辑器，用户
/// 无需理解/手动切换接管模式即可查看与修改（保存时内容有改动才落为 takeover）。
/// 视图按当前设置的 IPv6 开关计算（决定 AAAA 丢弃与默认 strategy）；`client.json` 缺失
/// 或损坏时回退默认（IPv6 关闭）。
#[tauri::command]
pub fn builtin_dns_slice_get(state: State<'_, AppState>) -> Result<DnsSlice, String> {
    let config_path = state.data_dir.join("client.json");
    let ipv6_enabled = if config_path.exists() {
        ClientConfig::load(&state.data_dir)
            .map_err(|e| format!("读取配置失败: {e}"))?
            .ipv6_enabled
    } else {
        false
    };
    Ok(builtin_dns_slice(ipv6_enabled))
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

        assert_eq!(
            view.route_rules.len(),
            2,
            "4 个直连规则集已合并为一条内置规则"
        );
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

    /// 内置 DNS 切片视图：IPv6 开关决定丢弃规则目标与默认 strategy；序列化字段名与
    /// client-core 的 `DnsSlice` TS 类型对齐（snake_case 原样）。
    #[test]
    fn builtin_dns_slice_view_shape() {
        let off = builtin_dns_slice(false);
        assert_eq!(off.rules[0].target, "HTTPS,SVCB,AAAA");
        assert_eq!(off.strategy.as_str(), "ipv4_only");
        assert!(off.reverse_mapping);
        assert_eq!(off.final_tag, "remote");

        let on = builtin_dns_slice(true);
        assert_eq!(on.rules[0].target, "HTTPS,SVCB");
        assert_eq!(on.strategy.as_str(), "prefer_ipv4");

        let value = serde_json::to_value(&off).unwrap();
        assert_eq!(value["servers"][0]["tag"], "local");
        assert_eq!(value["servers"][1]["server_type"], "https");
        assert_eq!(value["servers"][1]["detour"], "proxy");
        assert_eq!(value["rules"][1]["match_type"], "clash_mode");
        assert_eq!(value["rules"][1]["target"], "direct");
        assert_eq!(value["reverse_mapping"], true);
    }
}
