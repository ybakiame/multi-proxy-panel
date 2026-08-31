//! pp-client — 桌面客户端核心库。
//!
//! 提供客户端配置（[`config`]）、分享链接解析（[`share_link`]）、双核心节点转换
//! （[`node_convert`]）、通用订阅管理（[`subscription`]）、三方配置片段导入
//! （[`import`]）、Profile 模板与复写（[`profile`]）、核心配置合成（[`core_config`]）、
//! 系统代理（[`sysproxy`]）、核心运行器（[`runner`]）、核心引擎桥（[`core_engine`]）、
//! MITM 构建（[`mitm`]）与运行状态编排（[`state`]）。

#![allow(clippy::result_large_err)]

pub mod config;
pub mod core_config;
pub mod core_engine;
pub mod cores;
pub mod http_exec;
pub mod import;
pub mod local_override;
#[cfg(feature = "mitm")]
pub mod mitm;
pub mod node_convert;
pub mod privilege;
pub mod profile;
pub mod proxies;
pub mod remote;
pub mod runner;
pub mod share_link;
pub mod state;
pub mod subscription;
pub mod sysproxy;
pub mod validation;

pub use config::*;
pub use core_config::*;
pub use core_engine::*;
pub use cores::*;
pub use http_exec::*;
pub use import::*;
pub use local_override::*;
#[cfg(feature = "mitm")]
pub use mitm::*;
pub use node_convert::*;
pub use privilege::*;
pub use profile::*;
pub use proxies::*;
pub use remote::*;
pub use runner::*;
pub use share_link::*;
pub use state::*;
pub use subscription::*;
pub use sysproxy::*;
pub use validation::*;

/// 归一化 GitHub 资源 URL：`github.com/<owner>/<repo>/{blob,raw}/<branch>/<path>` →
/// `raw.githubusercontent.com/<owner>/<repo>/<branch>/<path>`；其他 URL 原样返回。
///
/// 供订阅拉取 / 远程资源拉取 / 导入脚本 URL / 远端嗅探在进入 HTTP 请求前统一调用，
/// 避免 GitHub 网页端 blob 链接被当作原始文件拉取。
pub fn normalize_resource_url(url: &str) -> String {
    let Some(rest) = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
    else {
        return url.to_string();
    };
    let mut parts = rest.splitn(5, '/');
    let (Some(owner), Some(repo), Some(kind), Some(branch)) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return url.to_string();
    };
    if owner.is_empty() || repo.is_empty() || branch.is_empty() {
        return url.to_string();
    }
    match kind {
        "blob" | "raw" => {
            let path = parts.next().unwrap_or("");
            let suffix = if path.is_empty() {
                String::new()
            } else {
                format!("/{path}")
            };
            format!("https://raw.githubusercontent.com/{owner}/{repo}/{branch}{suffix}")
        }
        _ => url.to_string(),
    }
}

/// GitHub 相关域名列表（含 `www.github.com`、`api.github.com` 与各类
/// `*.githubusercontent.com` 子域）。
const GITHUB_HOSTS: [&str; 8] = [
    "github.com",
    "www.github.com",
    "api.github.com",
    "raw.githubusercontent.com",
    "gist.github.com",
    "gist.githubusercontent.com",
    "codeload.github.com",
    "objects.githubusercontent.com",
];

/// 判断 URL 的 host 是否属于 GitHub 域名（忽略 scheme，支持带端口）。
///
/// 用于远程资源拉取时决定是否应用 GitHub 代理前缀 / 失败时提示。
pub fn is_github_url(url: &str) -> bool {
    let Some((_, rest)) = url.split_once("://") else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = host.split(':').next().unwrap_or(host);
    GITHUB_HOSTS.contains(&host)
}

/// 为 GitHub URL 拼接代理前缀：prefix 为空或非 GitHub URL 时原样返回；
/// 否则返回 `{prefix}/{url}`（prefix 尾部的 `/` 会被去重）。
pub fn apply_github_proxy_prefix(url: &str, prefix: &str) -> String {
    let prefix = prefix.trim();
    if prefix.is_empty() || !is_github_url(url) {
        return url.to_string();
    }
    format!("{}/{}", prefix.trim_end_matches('/'), url)
}

/// 构建远程资源拉取客户端：`fetch_via_local_proxy` 时经
/// `http://127.0.0.1:{mixed_port}` 本地核心 mixed 入站代理（构建失败回退无代理直连），
/// 否则 `.no_proxy()` 直连。
fn build_fetch_client(timeout: std::time::Duration, cfg: &config::ClientConfig) -> reqwest::Client {
    let no_proxy_client = || {
        reqwest::Client::builder()
            .timeout(timeout)
            .no_proxy()
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    };
    if cfg.fetch_via_local_proxy
        && let Ok(proxy) = reqwest::Proxy::all(format!("http://127.0.0.1:{}", cfg.mixed_port))
        && let Ok(client) = reqwest::Client::builder()
            .timeout(timeout)
            .proxy(proxy)
            .build()
    {
        return client;
    }
    // 本地代理客户端构建失败（非法地址等）→ 回退为无代理直连。
    no_proxy_client()
}

/// 读取响应体字节；非 2xx 视为失败（沿用既有 fetch 语义）。
async fn read_body_bytes(resp: reqwest::Response, url: &str) -> Result<bytes::Bytes, String> {
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("remote fetch returned HTTP {status} ({url})"));
    }
    resp.bytes()
        .await
        .map_err(|e| format!("failed to read remote body ({url}): {e}"))
}

/// 拉取单个 URL 的二进制内容；请求层错误（connect/timeout 等）重试一次，非 2xx 视为失败。
async fn fetch_url_bytes_with_retry(
    client: &reqwest::Client,
    url: &str,
) -> Result<bytes::Bytes, String> {
    match client.get(url).send().await {
        Ok(resp) => read_body_bytes(resp, url).await,
        // 请求层错误（connect/timeout 等）重试一次；仍失败以最后一次错误为准。
        Err(_) => match client.get(url).send().await {
            Ok(resp) => read_body_bytes(resp, url).await,
            Err(e) => Err(format!("remote fetch failed ({url}): {e}")),
        },
    }
}

/// 远程资源二进制拉取（Hub 订阅 / 脚本 / 图标共用入口）。
///
/// - 从 `data_dir/client.json` best-effort 读取 GitHub 访问设置（失败按默认值）
/// - URL 先经 [`normalize_resource_url`] 归一化，GitHub URL 再经
///   [`apply_github_proxy_prefix`] 拼接配置的代理前缀
/// - `fetch_via_local_proxy` 时请求经本机核心 mixed 端口代理，否则 `no_proxy()` 直连
/// - 请求层错误重试一次；最终失败且原始 URL 是 GitHub URL 时，错误信息追加
///   「设置 → GitHub 访问」的处理提示
pub async fn fetch_resource_bytes(
    data_dir: &std::path::Path,
    url: &str,
    timeout: std::time::Duration,
) -> pp_common::PanelResult<bytes::Bytes> {
    use pp_common::PanelError;

    let is_github = is_github_url(url);
    let cfg = config::ClientConfig::load(data_dir).unwrap_or_default();
    let request_url =
        apply_github_proxy_prefix(&normalize_resource_url(url), &cfg.github_proxy_prefix);
    let client = build_fetch_client(timeout, &cfg);

    match fetch_url_bytes_with_retry(&client, &request_url).await {
        Ok(bytes) => Ok(bytes),
        Err(e) => {
            if is_github {
                Err(PanelError::Client(format!(
                    "{e}（GitHub 直连失败：可在「设置 → GitHub 访问」中配置代理前缀或开启「走本地代理」后重试）"
                )))
            } else {
                Err(PanelError::Client(e))
            }
        }
    }
}

/// 远程资源文本拉取（Hub 订阅 / 脚本 / 嗅探共用入口），语义与
/// [`fetch_resource_bytes`] 一致（GitHub 代理前缀 / 走本地代理 / 重试 / 提示）。
///
/// 返回体按 UTF-8 解码；非 UTF-8 内容视为失败。
pub async fn fetch_resource_text(
    data_dir: &std::path::Path,
    url: &str,
    timeout: std::time::Duration,
) -> pp_common::PanelResult<String> {
    use pp_common::PanelError;

    let bytes = fetch_resource_bytes(data_dir, url, timeout).await?;
    String::from_utf8(bytes.to_vec()).map_err(|e| {
        PanelError::Client(format!("remote fetch returned non-UTF-8 body ({url}): {e}"))
    })
}

#[cfg(test)]
mod tests {
    use super::{
        apply_github_proxy_prefix, fetch_resource_text, is_github_url, normalize_resource_url,
    };

    #[test]
    fn is_github_url_matches_known_github_hosts() {
        assert!(is_github_url(
            "https://raw.githubusercontent.com/owner/repo/main/x.js"
        ));
        assert!(is_github_url("https://github.com/owner/repo"));
        assert!(is_github_url(
            "https://api.github.com/repos/owner/repo/releases"
        ));
        assert!(is_github_url("http://www.github.com/owner/repo"));
        assert!(is_github_url("https://gist.github.com/owner/abc123"));
        assert!(is_github_url(
            "https://gist.githubusercontent.com/owner/abc123/raw/x.js"
        ));
        assert!(is_github_url(
            "https://codeload.github.com/owner/repo/zip/main"
        ));
        assert!(is_github_url(
            "https://objects.githubusercontent.com/github-production/…"
        ));
        // 带端口 / query 仍识别 host。
        assert!(is_github_url(
            "https://raw.githubusercontent.com:443/o/r/main/x.js"
        ));
        // 非 GitHub 域名 / 无 scheme 判定为 false。
        assert!(!is_github_url("https://example.com/raw/x.js"));
        assert!(!is_github_url("https://github.com.evil.com/x"));
        assert!(!is_github_url("github.com/owner/repo"));
        assert!(!is_github_url(""));
    }

    #[test]
    fn apply_github_proxy_prefix_only_wraps_github_urls() {
        let prefix = "https://gh-proxy.com";
        assert_eq!(
            apply_github_proxy_prefix("https://raw.githubusercontent.com/o/r/main/x.js", prefix),
            "https://gh-proxy.com/https://raw.githubusercontent.com/o/r/main/x.js"
        );
        // prefix 尾部斜杠去重。
        assert_eq!(
            apply_github_proxy_prefix("https://github.com/o/r", "https://gh-proxy.com/"),
            "https://gh-proxy.com/https://github.com/o/r"
        );
        // api.github.com 同样走代理前缀（核心版本查询/下载共用此策略）。
        assert_eq!(
            apply_github_proxy_prefix("https://api.github.com/repos/o/r/releases", prefix),
            "https://gh-proxy.com/https://api.github.com/repos/o/r/releases"
        );
        // 非 GitHub URL 原样返回。
        assert_eq!(
            apply_github_proxy_prefix("https://example.com/x.js", prefix),
            "https://example.com/x.js"
        );
        // 空 prefix 原样返回。
        assert_eq!(
            apply_github_proxy_prefix("https://github.com/o/r", ""),
            "https://github.com/o/r"
        );
        assert_eq!(
            apply_github_proxy_prefix("https://github.com/o/r", "   "),
            "https://github.com/o/r"
        );
    }

    /// 本地 axum 服务测试 fetch_resource_text：200 成功 / 404 失败（非 GitHub URL，
    /// 不拼接代理前缀；默认配置直连）。
    #[tokio::test]
    async fn fetch_resource_text_succeeds_on_local_200_and_fails_on_404() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = axum::Router::new()
            .route("/ok", axum::routing::get(|| async { "hello" }))
            .route(
                "/missing",
                axum::routing::get(|| async { axum::http::StatusCode::NOT_FOUND }),
            );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let dir = tempfile::tempdir().unwrap();
        let timeout = std::time::Duration::from_secs(5);

        let ok = fetch_resource_text(dir.path(), &format!("http://{addr}/ok"), timeout)
            .await
            .unwrap();
        assert_eq!(ok, "hello");

        let err = fetch_resource_text(dir.path(), &format!("http://{addr}/missing"), timeout)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("HTTP 404"), "{err}");
    }

    #[test]
    fn normalizes_github_blob_and_raw_to_raw_githubusercontent() {
        assert_eq!(
            normalize_resource_url(
                "https://github.com/SagerNet/sing-box/blob/main/transport/splithttp/client.go"
            ),
            "https://raw.githubusercontent.com/SagerNet/sing-box/main/transport/splithttp/client.go"
        );
        assert_eq!(
            normalize_resource_url("https://github.com/owner/repo/raw/main/sub/module.sgmodule"),
            "https://raw.githubusercontent.com/owner/repo/main/sub/module.sgmodule"
        );
    }

    #[test]
    fn leaves_already_raw_and_non_github_urls_unchanged() {
        assert_eq!(
            normalize_resource_url("https://raw.githubusercontent.com/o/r/main/x.js"),
            "https://raw.githubusercontent.com/o/r/main/x.js"
        );
        assert_eq!(
            normalize_resource_url("https://example.com/not/github/blob/main/x.js"),
            "https://example.com/not/github/blob/main/x.js"
        );
        assert_eq!(
            normalize_resource_url("http://github.com/o/r/tree/main/x.js"),
            "http://github.com/o/r/tree/main/x.js"
        );
        // 结构不完整（缺 branch / path）时原样返回。
        assert_eq!(
            normalize_resource_url("https://github.com/owner/repo"),
            "https://github.com/owner/repo"
        );
    }
}
