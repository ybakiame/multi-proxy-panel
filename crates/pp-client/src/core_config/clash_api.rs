//! Clash API rule mode hot switch.

use std::time::Duration;

use pp_common::{PanelError, PanelResult};

/// Hot-switch rule mode via Clash API `PATCH /configs` (`rule` / `global` / `direct`).
///
/// Target: `http://127.0.0.1:{port}/configs`, body `{"mode": "<mode>"}`, 5 second timeout,
/// `no_proxy()` direct; when `secret` non-empty, includes `Authorization: Bearer <secret>`.
/// Connection failure / non-2xx returns `Err`.
///
/// Retries up to 3 times (500ms interval): Clash API may not be ready when core just started
/// (listen port not ready causing connection refused etc. transient failures), retry succeeds;
/// each failure logged at debug level, only returns `Err` when all fail (caller treats as
/// best-effort warning, not blocking: sing-box has no composition-level mode field, runtime
/// mode switching fully depends on this PATCH).
pub async fn push_clash_mode(port: u16, secret: &str, mode: &str) -> PanelResult<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .no_proxy()
        .build()
        .map_err(|e| PanelError::Client(format!("构建 Clash API 客户端失败: {e}")))?;
    let mut last_err: Option<PanelError> = None;
    for attempt in 1..=3 {
        let mut request = client
            .patch(format!("http://127.0.0.1:{port}/configs"))
            .json(&serde_json::json!({ "mode": mode }));
        if !secret.is_empty() {
            request = request.bearer_auth(secret);
        }
        match request.send().await {
            Ok(resp) if resp.status().is_success() => return Ok(()),
            Ok(resp) => {
                last_err = Some(PanelError::Client(format!(
                    "Clash API 推送规则模式失败（mode={mode}）: HTTP {}",
                    resp.status()
                )));
            }
            Err(e) => {
                last_err = Some(PanelError::Client(format!(
                    "Clash API 推送规则模式失败（mode={mode}）: {e}"
                )));
            }
        }
        if attempt < 3 {
            tracing::debug!(
                attempt,
                mode,
                error = %last_err.as_ref().expect("last_err set on failure"),
                "Clash API 推送规则模式失败，将重试"
            );
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
    Err(last_err.unwrap_or_else(|| {
        PanelError::Client(format!("Clash API 推送规则模式失败（mode={mode}）"))
    }))
}
