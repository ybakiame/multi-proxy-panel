//! Local Override Tauri commands.
//!
//! Provides frontend-facing commands for rule card management, template
//! application, and rule set subscription control.

use pp_client::local_override::{LocalOverrideStore, MarketManager, RuleSetManager};
use tauri::State;

use crate::state::AppState;

use super::convert::{convert_input_to_model, validate_local_override};
use super::views::*;

// ---------------------------------------------------------------------------

/// Build the full local override frontend view from `data_dir`.
///
/// 走 [`LocalOverrideStore::load`]：读取即触发幂等存量迁移（旧版内置订阅/模板
/// 记录归一化为 custom 模型）。提取成独立函数以便在无 Tauri 运行时下单测。
pub(crate) fn load_override_view(data_dir: &std::path::Path) -> Result<LocalOverrideView, String> {
    let store = LocalOverrideStore::new(data_dir.to_path_buf());
    let ovr = store
        .load()
        .map_err(|e| format!("failed to load local override: {e}"))?;
    let manager = RuleSetManager::new(data_dir.to_path_buf());
    let market = MarketManager::new(data_dir.to_path_buf());
    Ok(LocalOverrideView::from_model(&ovr, &manager, &market))
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
///
/// `market_sources` 不属于前端 save 契约（由专用市场命令管理）：保存时保留磁盘
/// 现值，避免在规则/规则集页编辑时清空市场源。
pub(crate) fn run_save(
    data_dir: &std::path::Path,
    input: SaveLocalOverrideInput,
) -> Result<(), String> {
    let mut ovr = convert_input_to_model(input)?;
    validate_local_override(&ovr).map_err(|e| format!("validation failed: {e}"))?;

    let store = LocalOverrideStore::new(data_dir.to_path_buf());
    let existing = store
        .load()
        .map_err(|e| format!("failed to load local override: {e}"))?;
    ovr.market_sources = existing.market_sources;
    // `remote_updated_at` 由后端 HEAD 维护、不在前端 save 契约中：按 id 从磁盘
    // 现值回填，避免整段替换落盘时把已探测到的远端更新时间清零。
    let existing_remote: std::collections::HashMap<&str, u64> = existing
        .custom_rule_sets
        .iter()
        .map(|rs| (rs.id.as_str(), rs.remote_updated_at))
        .collect();
    for rs in &mut ovr.custom_rule_sets {
        if let Some(remote) = existing_remote.get(rs.id.as_str()) {
            rs.remote_updated_at = *remote;
        }
    }

    let manager = RuleSetManager::new(data_dir.to_path_buf());
    manager
        .sync_custom_rule_set_files(&ovr.custom_rule_sets)
        .map_err(|e| format!("failed to persist custom rule set files: {e}"))?;

    store
        .save(&ovr)
        .map_err(|e| format!("failed to save local override: {e}"))
}

#[tauri::command]
pub fn local_override_save(
    state: State<'_, AppState>,
    input: SaveLocalOverrideInput,
) -> Result<(), String> {
    run_save(&state.data_dir, input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pp_client::local_override::{
        CustomRuleSet, CustomRuleSetSource, CustomTemplate, LocalOverride, LocalRule, RuleAction,
        RuleMatchType,
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
            rule_set_subscriptions: Vec::new(),
            applied_templates: Vec::new(),
            custom_rule_sets: vec![CustomRuleSet {
                id: "rs-1".to_string(),
                name: "my block list".to_string(),
                tag: "my-block".to_string(),
                source: CustomRuleSetSource::Manual {
                    content: r#"{"version":1,"rules":[{"domain_suffix":[".ads.example"]}]}"#
                        .to_string(),
                },
                last_updated: 0,
                remote_updated_at: 0,
            }],
            custom_templates: Vec::new(),
            market_sources: Vec::new(),
        };
        ovr.singbox.rules.push(sample_rule("r1", 0));
        ovr.custom_templates.push(CustomTemplate {
            id: "tpl-1".to_string(),
            name: "模板".to_string(),
            desc: "描述".to_string(),
            rules: vec![sample_rule("t1", 0).id],
            created_at: 1,
        });
        ovr
    }

    #[test]
    fn first_run_get_returns_empty_user_controlled_segments() {
        // 首次进入（local_override.json 缺失）：不再注入任何内置订阅，
        // 返回纯空的自定义段（custom 模型）。
        let dir = tempfile::tempdir().unwrap();
        let store = LocalOverrideStore::new(dir.path().to_path_buf());
        assert!(!store.override_file().exists());
        let view = load_override_view(dir.path()).unwrap();
        assert!(view.custom_rule_sets.is_empty());
        assert!(view.custom_templates.is_empty());
        assert!(view.applied_templates.is_empty());
        assert!(view.market_sources.is_empty());
        assert!(view.singbox.enabled);
        // 重复读取结果一致，不写盘、不注入内置数据。
        let view2 = load_override_view(dir.path()).unwrap();
        assert!(view2.custom_rule_sets.is_empty());
    }

    /// 前端 save 契约不含 `market_sources`：保存规则/规则集时必须保留磁盘上
    /// 已添加的市场源，且视图输出源列表 + 缓存条目数。
    #[test]
    fn save_preserves_market_sources_and_view_exposes_entry_count() {
        let dir = tempfile::tempdir().unwrap();
        let ovr = LocalOverride {
            market_sources: vec![pp_client::local_override::MarketSource {
                id: "m1".to_string(),
                name: "示例市场".to_string(),
                url: "https://e/market.json".to_string(),
                last_fetched: 7,
            }],
            ..Default::default()
        };
        let store = LocalOverrideStore::new(dir.path().to_path_buf());
        store.save(&ovr).unwrap();
        MarketManager::new(dir.path().to_path_buf())
            .write_cache(
                "m1",
                r#"[{"id":"ads","name":"广告","format":"binary","url":"https://e/ads.srs"}]"#,
            )
            .unwrap();

        // 走前端 save 契约（无 market_sources）→ 不应清空市场源。
        run_save(
            dir.path(),
            SaveLocalOverrideInput {
                singbox: CoreLocalOverrideInput {
                    rules: Vec::new(),
                    rule_sets: Vec::new(),
                    enabled: true,
                },
                applied_templates: Vec::new(),
                custom_rule_sets: Vec::new(),
                custom_templates: Vec::new(),
            },
        )
        .unwrap();
        let reloaded = store.load().unwrap();
        assert_eq!(reloaded.market_sources.len(), 1, "save 不得清空市场源");
        assert_eq!(reloaded.market_sources[0].id, "m1");

        let view = load_override_view(dir.path()).unwrap();
        assert_eq!(view.market_sources.len(), 1);
        assert_eq!(view.market_sources[0].last_fetched, 7);
        assert_eq!(view.market_sources[0].entry_count, 1);
    }

    #[test]
    fn save_then_get_roundtrip_preserves_custom_segments() {
        // 模拟保存（sync manual 落盘 + store.save）后 get 往返自洽：
        // custom 段（含 cached 状态判定）与模板快照原样回读。
        let dir = tempfile::tempdir().unwrap();
        let ovr = sample_override();
        let manager = pp_client::local_override::RuleSetManager::new(dir.path().to_path_buf());
        manager
            .sync_custom_rule_set_files(&ovr.custom_rule_sets)
            .unwrap();
        let store = LocalOverrideStore::new(dir.path().to_path_buf());
        store.save(&ovr).unwrap();

        let view = load_override_view(dir.path()).unwrap();
        assert_eq!(view.custom_rule_sets.len(), 1);
        let custom = &view.custom_rule_sets[0];
        assert_eq!(custom.tag, "my-block");
        assert!(custom.cached, "manual 内容同步落盘后 cached 应为 true");
        assert_eq!(view.custom_templates.len(), 1);
        assert_eq!(view.custom_templates[0].rules.len(), 1);
        assert_eq!(view.custom_templates[0].rules[0], "t1");
        assert_eq!(view.singbox.rules.len(), 1);
        assert!(view.singbox.enabled);
        // 写回文件里订阅段恒为空数组（序列化契约：字段保留 serde 兼容）。
        let reloaded = store.load().unwrap();
        assert!(reloaded.rule_set_subscriptions.is_empty());
    }

    #[test]
    fn load_override_view_migrates_legacy_builtin_to_custom() {
        // 旧版文件（已订阅内置 + 内置 applied 记录）经 load 迁移：
        // get 视图不再含订阅段，custom_rule_sets 含迁移出的 Remote（未缓存）；
        // 重复读取不重复迁移（幂等）。
        let dir = tempfile::tempdir().unwrap();
        let legacy = serde_json::json!({
            "singbox": { "rules": [], "rule_sets": [], "enabled": true },
            "rule_set_subscriptions": [{
                "id": "sub-geoip-cn",
                "community_id": "geoip-cn",
                "display_name": "GeoIP China",
                "category": "geoip",
                "subscribed": true,
                "singbox_url_template": "https://example.com/ip/{tag}.srs",
                "default_interval_minutes": 1440
            }],
            "applied_templates": [
                { "template_id": "return-china", "applied_at": 1, "generated_rule_ids": ["x"] }
            ],
            "custom_rule_sets": [],
            "custom_templates": []
        });
        std::fs::write(
            dir.path().join("local_override.json"),
            serde_json::to_string_pretty(&legacy).unwrap(),
        )
        .unwrap();

        let view = load_override_view(dir.path()).unwrap();
        assert_eq!(view.custom_rule_sets.len(), 1);
        let migrated = &view.custom_rule_sets[0];
        assert_eq!(migrated.tag, "geoip-cn");
        assert_eq!(migrated.name, "GeoIP China");
        assert_eq!(migrated.last_updated, 0);
        assert!(!migrated.cached, "迁移后尚未下载，无 backing 文件");
        assert!(view.applied_templates.is_empty());
        assert!(view.custom_templates.is_empty());

        // 幂等：二次读取不重复转换。
        let view2 = load_override_view(dir.path()).unwrap();
        assert_eq!(view2.custom_rule_sets.len(), 1);
        assert_eq!(view2.custom_rule_sets[0].id, migrated.id);
    }
}
