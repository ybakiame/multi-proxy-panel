//! Local Override 规则集命令。
//!
//! 自「废弃内置规则集订阅」起：
//! - `local_override_rulesets`（订阅状态列表）与 `local_override_toggle_ruleset`
//!   （订阅开关）**已删除**——前端统一消费 `local_override_get` 的
//!   `custom_rule_sets` 段（含 `cached` / `last_updated` / `remote_updated_at`），
//!   减少一条命令链路；
//! - 自「规则集移除 enabled」起 `local_override_update_rulesets_now` 语义收敛为
//!   「同步更新**全部** custom Remote」（纯资源刷新语义）：**await 全部下载完成
//!   后**保存并返回，确保 UI invalidate 重拉即见 cached / last_updated 变化。
//! - 自「规则集更新增强」起更新前先 HEAD 取 `Last-Modified` 做**智能跳过**（远端
//!   未更新则不下载），并新增 `local_override_update_rule_set` 单条更新；两者
//!   均返回 `{ updated, skipped, failed }` 汇总。

use pp_client::local_override::{
    LocalOverrideStore, RuleSetManager, RuleSetUpdateOutcome, RuleSetUpdateStatus,
};
use tauri::State;

use super::views::RuleSetUpdateOutcomeView;
use crate::state::AppState;

/// 将单条更新状态折叠为 1 条计数结果。
fn single_outcome(status: RuleSetUpdateStatus) -> RuleSetUpdateOutcome {
    match status {
        RuleSetUpdateStatus::Updated => RuleSetUpdateOutcome {
            updated: 1,
            ..Default::default()
        },
        RuleSetUpdateStatus::Skipped => RuleSetUpdateOutcome {
            skipped: 1,
            ..Default::default()
        },
        RuleSetUpdateStatus::Failed => RuleSetUpdateOutcome {
            failed: 1,
            ..Default::default()
        },
    }
}

/// 全部更新命令实现体（抽取以便无 Tauri 运行时单测）：同步 await 智能更新全部
/// custom Remote，保存到 `data_dir`，返回 updated / skipped / failed 汇总。
pub(crate) async fn run_update_rulesets_now(
    data_dir: &std::path::Path,
) -> Result<RuleSetUpdateOutcomeView, String> {
    let store = LocalOverrideStore::new(data_dir.to_path_buf());
    let mut ovr = store
        .load()
        .map_err(|e| format!("failed to load local override: {e}"))?;

    let manager = RuleSetManager::new(data_dir.to_path_buf());
    let outcome = manager
        .update_custom_remotes(&mut ovr)
        .await
        .map_err(|e| format!("failed to update rule sets: {e}"))?;

    store
        .save(&ovr)
        .map_err(|e| format!("failed to save after update: {e}"))?;

    Ok(outcome.into())
}

/// 单条更新命令实现体：按 `id` 找到 custom 规则集并智能更新，保存后返回 1 条计数。
pub(crate) async fn run_update_rule_set(
    data_dir: &std::path::Path,
    id: &str,
) -> Result<RuleSetUpdateOutcomeView, String> {
    let store = LocalOverrideStore::new(data_dir.to_path_buf());
    let mut ovr = store
        .load()
        .map_err(|e| format!("failed to load local override: {e}"))?;

    let manager = RuleSetManager::new(data_dir.to_path_buf());
    let Some(rs) = ovr.custom_rule_sets.iter_mut().find(|rs| rs.id == id) else {
        return Err(format!("rule set not found: {id}"));
    };
    let outcome = single_outcome(manager.update_custom_rule_set(rs).await);

    store
        .save(&ovr)
        .map_err(|e| format!("failed to save after update: {e}"))?;

    Ok(outcome.into())
}

/// Manually update all Remote custom rule sets now (smart skip).
///
/// 同步 await 智能更新并刷新 `last_updated` / `remote_updated_at` 后落盘；单条
/// HEAD / 下载失败 best-effort（仅告警，不中断整批），返回 updated / skipped /
/// failed 汇总。
#[tauri::command]
pub async fn local_override_update_rulesets_now(
    state: State<'_, AppState>,
) -> Result<RuleSetUpdateOutcomeView, String> {
    run_update_rulesets_now(&state.data_dir).await
}

/// Update a single custom rule set by id (smart skip).
///
/// 与全部更新同一套智能跳过语义；返回 `{ updated, skipped, failed }`（恰有一项
/// 为 1）。规则集不存在时报错。
#[tauri::command]
pub async fn local_override_update_rule_set(
    state: State<'_, AppState>,
    id: String,
) -> Result<RuleSetUpdateOutcomeView, String> {
    run_update_rule_set(&state.data_dir, &id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::header;

    async fn spawn_server() -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = axum::Router::new().route(
            "/ok.srs",
            axum::routing::get(|| async { "binary-remote-body" }).head(|| async {
                (
                    [(header::LAST_MODIFIED, "Sun, 06 Nov 1994 08:49:37 GMT")],
                    "",
                )
            }),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}/ok.srs")
    }

    fn sample_set(
        id: &str,
        url: String,
        last_updated: u64,
    ) -> pp_client::local_override::CustomRuleSet {
        pp_client::local_override::CustomRuleSet {
            id: id.to_string(),
            name: "GeoIP CN".to_string(),
            tag: "geoip-cn".to_string(),
            source: pp_client::local_override::CustomRuleSetSource::Remote {
                url,
                format: pp_client::local_override::RuleSetFormat::Binary,
            },
            last_updated,
            remote_updated_at: 0,
        }
    }

    /// 全部更新链路：智能下载 → last_updated / remote_updated_at 刷新 → save 落盘；
    /// 重读即可见 cached / 时间戳变化。
    #[tokio::test]
    async fn update_now_downloads_awaits_and_persists_last_updated() {
        let url = spawn_server().await;
        let dir = tempfile::tempdir().unwrap();
        let ovr = pp_client::local_override::LocalOverride {
            custom_rule_sets: vec![sample_set("c-1", url, 0)],
            ..Default::default()
        };
        let store = LocalOverrideStore::new(dir.path().to_path_buf());
        store.save(&ovr).unwrap();

        let outcome = run_update_rulesets_now(dir.path()).await.unwrap();
        assert_eq!(outcome.updated, 1);
        assert_eq!(outcome.skipped, 0);
        assert_eq!(outcome.failed, 0);

        // save 已落盘：重新 load 可见时间戳刷新，backing 文件已缓存。
        let reloaded = store.load().unwrap();
        let rs = &reloaded.custom_rule_sets[0];
        assert!(rs.last_updated > 0, "last_updated 应在下载成功后刷新");
        assert_eq!(rs.remote_updated_at, 784_111_777);
        let manager = RuleSetManager::new(dir.path().to_path_buf());
        let path = manager
            .custom_rule_set_file_path(&rs.id, pp_client::local_override::RuleSetFormat::Binary);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "binary-remote-body",
            "下载完成后 backing 文件应存在（UI 重拉即见 cached=true）"
        );
    }

    /// 单条更新链路：远端未更新时跳过（不重下）、已更新时下载；不存在报错。
    #[tokio::test]
    async fn update_single_rule_set_skips_when_remote_unchanged() {
        let url = spawn_server().await;
        let dir = tempfile::tempdir().unwrap();
        // last_updated 晚于远端 Last-Modified → 跳过。
        let ovr = pp_client::local_override::LocalOverride {
            custom_rule_sets: vec![sample_set("c-1", url, 2_000_000_000)],
            ..Default::default()
        };
        let store = LocalOverrideStore::new(dir.path().to_path_buf());
        store.save(&ovr).unwrap();

        let outcome = run_update_rule_set(dir.path(), "c-1").await.unwrap();
        assert_eq!(
            (outcome.updated, outcome.skipped, outcome.failed),
            (0, 1, 0)
        );
        let reloaded = store.load().unwrap();
        assert_eq!(reloaded.custom_rule_sets[0].last_updated, 2_000_000_000);
        assert_eq!(reloaded.custom_rule_sets[0].remote_updated_at, 784_111_777);

        // 未知 id 报错。
        assert!(run_update_rule_set(dir.path(), "nope").await.is_err());
    }
}
