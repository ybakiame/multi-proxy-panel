//! Scenario templates: user-defined custom templates ([`CustomTemplate`]).
//!
//! 内置场景模板机制（return-china / overseas / ad-filter）已废弃；`apply_template`
//! 只接受 `"custom:<id>"` 形式的用户自定义模板地址（[`CUSTOM_TEMPLATE_PREFIX`]）。
//!
//! # 语义（引用 + 应用激活）
//!
//! 模板只保存**规则 ID 引用列表**（[`CustomTemplate::rules`]，不复制规则定义）。
//!
//! - **应用**（[`apply_template`]）只往 [`AppliedTemplate`] 加一条记录
//!   （`template_id + applied_at`），不触碰 `singbox.rules`；再次应用幂等（不
//!   重复记录，返回 Ok）。
//! - **撤销**（[`revert_template`]）只移除这条记录，规则列表不动。
//! - **注入**（state 层）以 `applied_templates` ∩ 模板引用集合计算激活规则 ID，
//!   只注入 `enabled && id ∈ 激活集合` 的规则；多模板叠加 = 引用并集。
//! - 规则禁用/删除时模板引用**保留**，应用时自动跳过失效引用；
//!   [`template_invalid_refs`] 给出失效集合供 View 展示失效数。
//!
//! ADR-0002, section 3.3.

use std::collections::HashSet;

use pp_common::{PanelError, PanelResult};

use super::{AppliedTemplate, CustomTemplate, LocalOverride};

/// Prefix distinguishing a user-defined template ID from built-in ones.
///
/// Apply / revert commands receive `"custom:<id>"` as the `template_id`.
pub const CUSTOM_TEMPLATE_PREFIX: &str = "custom:";

/// Apply a scenario template to the given [`LocalOverride`].
///
/// Custom templates (`template_id = "custom:<id>"`, [`CUSTOM_TEMPLATE_PREFIX`]):
/// validates the template exists, then records an [`AppliedTemplate`] entry
/// (`template_id = "custom:<id>"`). **No rules are copied.**
///
/// Re-applying an already applied template is an **idempotent no-op**: it does
/// not add a duplicate record and returns the template's current rule ID
/// references.
///
/// Returns the template's rule ID reference list on success.
///
/// # Errors
///
/// Returns error if `template_id` is not `"custom:<id>"` or the custom template
/// does not exist.
pub fn apply_template(
    ovr: &mut LocalOverride,
    template_id: &str,
    now_sec: u64,
) -> PanelResult<Vec<String>> {
    let Some(custom_id) = template_id.strip_prefix(CUSTOM_TEMPLATE_PREFIX) else {
        return Err(PanelError::Client(format!(
            "unknown template id '{template_id}' (only custom templates, addressed as '{CUSTOM_TEMPLATE_PREFIX}<id>', are supported)"
        )));
    };

    let template: CustomTemplate = ovr
        .custom_templates
        .iter()
        .find(|t| t.id == custom_id)
        .cloned()
        .ok_or_else(|| PanelError::Client(format!("custom template not found: {custom_id}")))?;

    // Idempotent re-apply: no duplicate record.
    if !ovr
        .applied_templates
        .iter()
        .any(|t| t.template_id == template_id)
    {
        ovr.applied_templates.push(AppliedTemplate {
            template_id: template_id.to_string(),
            applied_at: now_sec,
            // generated_rule_ids is deprecated under the reference semantics
            // (kept only for serde compatibility); always write empty.
            generated_rule_ids: Vec::new(),
        });
    }

    Ok(template.rules)
}

/// Revert (undo) a previously applied template by its ID.
///
/// Removes only the [`AppliedTemplate`] record — **rules are never copied or
/// removed** under the reference semantics.
///
/// Returns `true` if an applied record was found and removed.
pub fn revert_template(ovr: &mut LocalOverride, template_id: &str) -> bool {
    let before = ovr.applied_templates.len();
    ovr.applied_templates
        .retain(|t| t.template_id != template_id);
    ovr.applied_templates.len() != before
}

/// Collect the currently **active** rule IDs for local override injection.
///
/// Active set = union of the rule ID reference lists of every custom template
/// that has an [`AppliedTemplate`] record. Applied records whose template no
/// longer exists contribute nothing (their referenced rules fall out of the
/// active set — the config simply stops injecting them).
pub fn active_rule_ids(ovr: &LocalOverride) -> HashSet<String> {
    let mut ids = HashSet::new();
    for record in &ovr.applied_templates {
        let Some(custom_id) = record.template_id.strip_prefix(CUSTOM_TEMPLATE_PREFIX) else {
            continue;
        };
        if let Some(tpl) = ovr.custom_templates.iter().find(|t| t.id == custom_id) {
            ids.extend(tpl.rules.iter().cloned());
        }
    }
    ids
}

/// References of `template` that are currently **invalid** (stale).
///
/// A reference is invalid when its rule ID does not exist in `singbox.rules` or
/// the matching rule is `disabled`. Invalid references are intentionally kept on
/// the template (option b: rule disable/delete never edits templates) and
/// skipped at injection time; this function reports them for the View's
/// invalid-count display.
pub fn template_invalid_refs(ovr: &LocalOverride, template: &CustomTemplate) -> Vec<String> {
    template
        .rules
        .iter()
        .filter(|id| !ovr.singbox.rules.iter().any(|r| &r.id == *id && r.enabled))
        .cloned()
        .collect()
}

#[cfg(test)]
#[path = "tests/template_tests.rs"]
mod tests;
