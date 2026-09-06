//! Local Override Tauri commands.
//!
//! Provides frontend-facing commands for rule card management, template
//! application, and rule set subscription control.

use pp_client::local_override::{LocalOverrideStore, RuleSetManager};
use tauri::State;

use crate::state::AppState;

use super::convert::{convert_input_to_model, validate_local_override};
use super::views::*;

// ---------------------------------------------------------------------------

/// Build the full local override frontend view from `data_dir`.
///
/// Every local-override read path must go through
/// [`LocalOverrideStore::ensure_builtin_subscriptions`] (idempotent: initializes
/// and persists the built-in 5 rule-set subscriptions when the list is empty),
/// so the rule-set management UI shows the built-in list on first entry even
/// before any write/update command ran. Extracted from the `#[tauri::command]`
/// so this first-run contract is unit-testable without a Tauri runtime.
pub(crate) fn load_override_view(data_dir: &std::path::Path) -> Result<LocalOverrideView, String> {
    let store = LocalOverrideStore::new(data_dir.to_path_buf());
    let ovr = store
        .ensure_builtin_subscriptions()
        .map_err(|e| format!("failed to load local override: {e}"))?;
    let manager = RuleSetManager::new(data_dir.to_path_buf());
    Ok(LocalOverrideView::from_model(&ovr, &manager))
}

/// Get full local override config.
#[tauri::command]
pub fn local_override_get(state: State<'_, AppState>) -> Result<LocalOverrideView, String> {
    load_override_view(&state.data_dir)
}

/// Save full local override config (frontend edits).
///
/// The `custom_rule_sets` segment is full-replacement (same semantics as
/// `rules`). Manual contents are persisted to disk and backing files of
/// removed / switched custom rule sets are best-effort cleaned before the
/// JSON is written, so injected local rule_set paths always resolve.
#[tauri::command]
pub fn local_override_save(
    state: State<'_, AppState>,
    input: SaveLocalOverrideInput,
) -> Result<(), String> {
    let ovr = convert_input_to_model(input)?;
    validate_local_override(&ovr).map_err(|e| format!("validation failed: {e}"))?;

    let manager = RuleSetManager::new(state.data_dir.clone());
    manager
        .sync_custom_rule_set_files(&ovr.custom_rule_sets)
        .map_err(|e| format!("failed to persist custom rule set files: {e}"))?;

    let store = LocalOverrideStore::new(state.data_dir.clone());
    store
        .save(&ovr)
        .map_err(|e| format!("failed to save local override: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pp_client::local_override::{
        CustomRuleSet, CustomRuleSetSource, CustomTemplate, LocalOverride, LocalRule, RuleAction,
        RuleMatchType, RuleSetSubscription, built_in_rule_set_subscriptions,
    };

    fn sample_rule(id: &str, sort_order: i32) -> LocalRule {
        LocalRule {
            id: id.to_string(),
            name: String::new(),
            enabled: true,
            match_type: RuleMatchType::Domain,
            target: "example.com".to_string(),
            action: RuleAction::Direct,
            advanced: Default::default(),
            note: String::new(),
            created_at: 1,
            sort_order,
        }
    }

    /// An override carrying every user-editable segment (custom rule set +
    /// custom template + rules), shaped like a post-edit save.
    fn sample_override() -> LocalOverride {
        let mut ovr = LocalOverride {
            singbox: Default::default(),
            rule_set_subscriptions: built_in_rule_set_subscriptions(),
            applied_templates: Vec::new(),
            custom_rule_sets: vec![CustomRuleSet {
                id: "rs-1".to_string(),
                name: "my block list".to_string(),
                tag: "my-block".to_string(),
                source: CustomRuleSetSource::Manual {
                    content: r#"{"version":1,"rules":[{"domain_suffix":[".ads.example"]}]}"#
                        .to_string(),
                },
                enabled: true,
                last_updated: 0,
            }],
            custom_templates: Vec::new(),
        };
        ovr.singbox.rules.push(sample_rule("r1", 0));
        ovr.custom_templates.push(CustomTemplate {
            id: "tpl-1".to_string(),
            name: "模板".to_string(),
            desc: "描述".to_string(),
            rules: vec![sample_rule("t1", 0)],
            created_at: 1,
        });
        ovr
    }

    #[test]
    fn first_run_get_returns_builtin_subscriptions() {
        // 首次进入（local_override.json 缺失）：读命令必须经 ensure 初始化内置 5 订阅，
        // 而不是返回空列表（历史 bug：仅「立即更新」等写路径触发初始化）。
        let dir = tempfile::tempdir().unwrap();
        let store = LocalOverrideStore::new(dir.path().to_path_buf());
        assert!(!store.override_file().exists());
        let view = load_override_view(dir.path()).unwrap();
        assert_eq!(view.rule_set_subscriptions.len(), 5);
        assert!(view.custom_rule_sets.is_empty());
        assert!(view.custom_templates.is_empty());
        assert!(view.singbox.enabled);
        // ensure 幂等：重复读取不重复追加（列表非空时直接返回）。
        let view2 = load_override_view(dir.path()).unwrap();
        assert_eq!(view2.rule_set_subscriptions.len(), 5);
        // 初始化结果已持久化，后续真实保存/读取不会丢失。
        let reloaded = store.load().unwrap();
        assert_eq!(reloaded.rule_set_subscriptions.len(), 5);
    }

    #[test]
    fn save_then_get_roundtrip_preserves_custom_segments() {
        // 模拟保存（sync manual 落盘 + store.save）后 get 往返自洽：
        // 内置订阅不回退为空、custom 段（含 cached 状态判定）与模板快照原样回读。
        let dir = tempfile::tempdir().unwrap();
        let ovr = sample_override();
        let manager = pp_client::local_override::RuleSetManager::new(dir.path().to_path_buf());
        manager
            .sync_custom_rule_set_files(&ovr.custom_rule_sets)
            .unwrap();
        LocalOverrideStore::new(dir.path().to_path_buf())
            .save(&ovr)
            .unwrap();

        let view = load_override_view(dir.path()).unwrap();
        assert_eq!(view.rule_set_subscriptions.len(), 5);
        assert_eq!(view.custom_rule_sets.len(), 1);
        let custom = &view.custom_rule_sets[0];
        assert_eq!(custom.tag, "my-block");
        assert!(custom.cached, "manual 内容同步落盘后 cached 应为 true");
        assert_eq!(view.custom_templates.len(), 1);
        assert_eq!(view.custom_templates[0].rules.len(), 1);
        assert_eq!(view.singbox.rules.len(), 1);
        assert!(view.singbox.enabled);
        // 未订阅的社区订阅在往返后仍保持未订阅（无状态漂移）。
        assert!(view.rule_set_subscriptions.iter().all(|s| !s.subscribed));
        // 内置 id 完整（序列化契约），category 为小写字符串。
        assert!(
            view.rule_set_subscriptions
                .iter()
                .any(|s| s.community_id == "geoip-cn" && s.category == "geoip")
        );
    }

    #[test]
    fn existing_nonempty_file_is_not_duplicated() {
        // 用户已有订阅（列表非空）：ensure 不重复注入内置项。
        let dir = tempfile::tempdir().unwrap();
        let store = LocalOverrideStore::new(dir.path().to_path_buf());
        let mut ovr = sample_override();
        ovr.rule_set_subscriptions = vec![RuleSetSubscription {
            id: "sub-custom".to_string(),
            community_id: "my-community".to_string(),
            display_name: "自定义".to_string(),
            category: pp_client::local_override::RuleSetCategory::Custom,
            subscribed: false,
            singbox_url_template: "https://example.com/{tag}.srs".to_string(),
            default_interval_minutes: 1440,
        }];
        store.save(&ovr).unwrap();
        let view = load_override_view(dir.path()).unwrap();
        assert_eq!(view.rule_set_subscriptions.len(), 1);
        assert_eq!(view.rule_set_subscriptions[0].community_id, "my-community");
    }
}
