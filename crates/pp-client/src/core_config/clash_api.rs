//! Clash API rule mode hot switch.

use std::time::Duration;

use pp_common::{PanelError, PanelResult};

/// Hot-switch rule mode via Clash API `PATCH /configs` (`rule` / `global` / `direct`).
///
/// Target: `http://127.0.0.1:{port}/configs`, body `{"mode": "<mode>"}`, 5 second timeout,
/// `no_proxy()` direct; when `secret` non-empty, includes `Authorization: Bearer <secret>`.
/// Connection failure / non-2xx returns `Err`.
///
/// Retries with exponential backoff (0.5s, 1s, 2s, 4s, 8s; ~15s total): Clash API may
/// not be ready when core just started (especially on Android where the VPN service
/// boots asynchronously), longer window lets it come up; each failure logged at debug
/// level, only returns `Err` when all fail (caller treats as best-effort warning, not
/// blocking: sing-box has no composition-level mode field, runtime mode switching fully
/// depends on this PATCH).
pub async fn push_clash_mode(port: u16, secret: &str, mode: &str) -> PanelResult<()> {
    const BACKOFF_MS: [u64; 5] = [500, 1000, 2000, 4000, 8000];

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .no_proxy()
        .build()
        .map_err(|e| PanelError::Client(format!("构建 Clash API 客户端失败: {e}")))?;
    let mut last_err: Option<PanelError> = None;
    for (idx, backoff) in BACKOFF_MS.iter().enumerate() {
        let attempt = idx + 1;
        let mut request = client
            .patch(format!("http://127.0.0.1:{port}/configs"))
            .json(&serde_json::json!({ "mode": mode }));
        if !secret.is_empty() {
            request = request.bearer_auth(secret);
        }
        match request.send().await {
            Ok(resp) if resp.status().is_success() => {
                if attempt > 1 {
                    tracing::info!(attempt, mode, "Clash API 推送规则模式重试后成功");
                }
                return Ok(());
            }
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
        if idx + 1 < BACKOFF_MS.len() {
            tracing::debug!(
                attempt,
                mode,
                backoff_ms = backoff,
                error = %last_err.as_ref().expect("last_err set on failure"),
                "Clash API 推送规则模式失败，将重试"
            );
            tokio::time::sleep(Duration::from_millis(*backoff)).await;
        }
    }
    Err(last_err.unwrap_or_else(|| {
        PanelError::Client(format!("Clash API 推送规则模式失败（mode={mode}）"))
    }))
}

/// Poll `GET /version` until the Clash API answers or `max_wait` elapses.
///
/// Used before one-shot runtime calls (mode push, selection replay) so they
/// don't race a core that is still starting (notably Android VPN service boot).
pub async fn wait_clash_api_ready(port: u16, secret: &str, max_wait: Duration) -> PanelResult<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .no_proxy()
        .build()
        .map_err(|e| PanelError::Client(format!("构建 Clash API 客户端失败: {e}")))?;
    let started = std::time::Instant::now();
    let mut backoff = Duration::from_millis(250);
    loop {
        let mut request = client.get(format!("http://127.0.0.1:{port}/version"));
        if !secret.is_empty() {
            request = request.bearer_auth(secret);
        }
        match request.send().await {
            Ok(resp) if resp.status().is_success() => return Ok(()),
            _ => {
                if started.elapsed() >= max_wait {
                    return Err(PanelError::Client(format!(
                        "Clash API 在 {:?} 内未就绪",
                        max_wait
                    )));
                }
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(Duration::from_secs(2));
            }
        }
    }
}
