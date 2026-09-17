//! 内置分组字段覆写（从 `apply` 拆出以控制文件规模，见
//! `.agents/rules/code-organization.md`）。

use serde_json::{Map, Value};

use super::outbound::OutboundProtocol;
use super::{ApplyReport, CustomOutbound, str_value};

/// 内置分组字段覆写（见 [`CustomOutbound::builtin`] 字段文档）。
///
/// 按 `name` 匹配模板内置分组（`proxy` selector / `auto` urltest），覆写可调字段：
/// - selector：`default`（非空且属于当前成员列表时才生效，避免订阅变更后引用悬空）、
///   `interrupt_exist_connections`（true 才覆写，false = 模板默认）；
/// - urltest：`url` / `interval`（非空）、`tolerance`（> 0）、`interrupt_exist_connections`
///   （true 才覆写）。
///
/// 成员列表不可经切片编辑（模板动态计算：订阅节点变化跟随）。
pub(super) fn apply_builtin_group_override(
    obj: &mut Map<String, Value>,
    item: &CustomOutbound,
    _report: &mut ApplyReport,
) {
    let Some(outbounds) = obj.get_mut("outbounds").and_then(Value::as_array_mut) else {
        return;
    };
    let Some(target) = outbounds
        .iter_mut()
        .find(|o| o.get("tag").and_then(Value::as_str) == Some(item.name.as_str()))
    else {
        tracing::debug!(name = %item.name, "内置分组覆写：模板中无同名分组，跳过");
        return;
    };
    let Some(target_obj) = target.as_object_mut() else {
        return;
    };
    match &item.protocol {
        OutboundProtocol::Selector(sel) => {
            // 静态成员分组（global/final）：成员可由切片编辑，非空才覆写（模板动态分组的
            // 成员不可经切片编辑）。
            let is_static = crate::core_config::BUILTIN_OUTBOUND_GROUPS
                .iter()
                .any(|spec| spec.name == item.name && !spec.dynamic_members);
            if is_static && !sel.outbounds.is_empty() {
                target_obj.insert(
                    "outbounds".to_string(),
                    Value::Array(sel.outbounds.iter().map(|m| str_value(m)).collect()),
                );
            }
            let members: Vec<&str> = target_obj
                .get("outbounds")
                .and_then(Value::as_array)
                .map(|arr| arr.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            if !sel.default.is_empty() && members.contains(&sel.default.as_str()) {
                target_obj.insert("default".to_string(), str_value(&sel.default));
            } else if !sel.default.is_empty() {
                tracing::warn!(
                    default = %sel.default,
                    "内置 selector 覆写：default 不在当前成员列表（订阅可能已变更），保持模板默认"
                );
            }
            if sel.interrupt_exist_connections {
                target_obj.insert("interrupt_exist_connections".to_string(), Value::Bool(true));
            }
        }
        OutboundProtocol::UrlTest(ut) => {
            if !ut.url.is_empty() {
                target_obj.insert("url".to_string(), str_value(&ut.url));
            }
            if !ut.interval.is_empty() {
                target_obj.insert("interval".to_string(), str_value(&ut.interval));
            }
            if ut.tolerance > 0 {
                target_obj.insert("tolerance".to_string(), Value::from(ut.tolerance));
            }
            if ut.interrupt_exist_connections {
                target_obj.insert("interrupt_exist_connections".to_string(), Value::Bool(true));
            }
        }
        _ => {}
    }
}
