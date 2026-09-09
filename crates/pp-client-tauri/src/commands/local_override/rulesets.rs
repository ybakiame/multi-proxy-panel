//! Local Override 规则集命令。
//!
//! 自「废弃内置规则集订阅」起：
//! - `local_override_rulesets`（订阅状态列表）与 `local_override_toggle_ruleset`
//!   （订阅开关）**已删除**——前端统一消费 `local_override_get` 的
//!   `custom_rule_sets` 段（含 `cached` / `last_updated`），减少一条命令链路；
//! - 自「规则集移除 enabled」起 `local_override_update_rulesets_now` 语义收敛为
//!   「同步更新**全部** custom Remote」（纯资源刷新语义）：**await 全部下载完成
//!   后**保存并返回，确保 UI invalidate 重拉即见 cached / last_updated 变化。

use pp_client::local_override::{LocalOverrideStore, RuleSetManager};
use tauri::State;

use crate::state::AppState;

/// 更新命令实现体（抽取以便无 Tauri 运行时单测）：同步 await 下载、刷新
/// `last_updated` 并保存到 `data_dir`，返回成功更新的数量。
pub(crate) async fn run_update_rulesets_now(data_dir: &std::path::Path) -> Result<usize, String> {
    let store = LocalOverrideStore::new(data_dir.to_path_buf());
    let mut ovr = store
        .load()
        .map_err(|e| format!("failed to load local override: {e}"))?;

    let manager = RuleSetManager::new(data_dir.to_path_buf());
    let updated = manager
        .update_custom_remotes(&mut ovr)
        .await
        .map_err(|e| format!("failed to update rule sets: {e}"))?;

    store
        .save(&ovr)
        .map_err(|e| format!("failed to save after update: {e}"))?;

    Ok(updated)
}

/// Manually update all Remote custom rule sets now.
///
/// 同步 await 下载并刷新 `last_updated` 后落盘；失败条目 best-effort（仅告警，
/// 不中断整批），返回成功更新的数量。
#[tauri::command]
pub async fn local_override_update_rulesets_now(
    state: State<'_, AppState>,
) -> Result<usize, String> {
    run_update_rulesets_now(&state.data_dir).await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 更新命令链路：download await 完成 → last_updated 刷新 → save 落盘；
    /// 重读即可见 cached / last_updated 变化。
    #[tokio::test]
    async fn update_now_downloads_awaits_and_persists_last_updated() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = axum::Router::new().route(
            "/ok.srs",
            axum::routing::get(|| async { "binary-remote-body" }),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let dir = tempfile::tempdir().unwrap();
        // 预置一个 custom Remote 并落盘 override 文件（纯资源：无 enabled）。
        let ovr = pp_client::local_override::LocalOverride {
            custom_rule_sets: vec![pp_client::local_override::CustomRuleSet {
                id: "c-1".to_string(),
                name: "GeoIP CN".to_string(),
                tag: "geoip-cn".to_string(),
                source: pp_client::local_override::CustomRuleSetSource::Remote {
                    url: format!("http://{addr}/ok.srs"),
                    format: pp_client::local_override::RuleSetFormat::Binary,
                },
                last_updated: 0,
            }],
            ..Default::default()
        };
        let store = LocalOverrideStore::new(dir.path().to_path_buf());
        store.save(&ovr).unwrap();

        let updated = run_update_rulesets_now(dir.path()).await.unwrap();
        assert_eq!(updated, 1);

        // save 已落盘：重新 load 可见 last_updated 刷新，backing 文件已缓存。
        let reloaded = store.load().unwrap();
        let rs = &reloaded.custom_rule_sets[0];
        assert!(rs.last_updated > 0, "last_updated 应在下载成功后刷新");
        let manager = RuleSetManager::new(dir.path().to_path_buf());
        let path = manager
            .custom_rule_set_file_path(&rs.id, pp_client::local_override::RuleSetFormat::Binary);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "binary-remote-body",
            "下载完成后 backing 文件应存在（UI 重拉即见 cached=true）"
        );
    }
}
