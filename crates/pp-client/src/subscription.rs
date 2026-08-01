//! 通用订阅管理：多订阅存储（`data_dir/subscriptions.json`）、格式嗅探与 URL 拉取。
//!
//! 订阅不再绑定 Hub：`fetch_subscription` 接受任意订阅 URL，按内容嗅探
//! 分享链接 / clash YAML / sing-box JSON 三种格式，统一产出双核心节点。
//! 旧版 Hub 路径（`/sub/{token}?format=...`）保留为 [`SubscriptionFetcher`]
//! 的兼容方法，供 `state` 在未配置通用订阅时回退使用。

use std::path::PathBuf;
use std::time::Duration;

use pp_common::{PanelError, PanelResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::node_convert::{mihomo_to_singbox, singbox_to_mihomo};
use crate::profile::{extract_nodes_mihomo, extract_nodes_singbox};
use crate::share_link::{ShareLinkParseResult, parse_share_links};

/// 订阅用户信息，来自 `subscription-userinfo` 响应头。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionInfo {
    /// 已用上行字节数。
    pub upload: Option<u64>,
    /// 已用下行字节数。
    pub download: Option<u64>,
    /// 总流量字节数。
    pub total: Option<u64>,
    /// 到期时间戳（秒）。
    pub expire: Option<u64>,
}

/// 一条订阅。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subscription {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    pub enabled: bool,
    pub userinfo: Option<SubscriptionInfo>,
    /// 最近一次 fetch 成功的节点数（sing-box 侧可用节点数）；`0` = 尚未成功拉取。
    ///
    /// `#[serde(default)]` 保证旧版 `subscriptions.json`（无此字段）可正常反序列化。
    #[serde(default)]
    pub node_count: u64,
    /// 最近一次 fetch 的错误信息（失败时记录，不阻塞已有数据展示）。
    ///
    /// `#[serde(default)]` 保证旧版 `subscriptions.json`（无此字段）可正常反序列化。
    #[serde(default)]
    pub error: Option<String>,
}

/// 订阅存储：读写 `data_dir/subscriptions.json`（load / save / add / remove / set_enabled）。
#[derive(Debug, Clone)]
pub struct SubscriptionStore {
    data_dir: PathBuf,
}

impl SubscriptionStore {
    /// 基于数据目录创建存储。
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    /// `data_dir/subscriptions.json`。
    pub fn file(&self) -> PathBuf {
        self.data_dir.join("subscriptions.json")
    }

    /// 读取订阅列表；文件缺失时返回空列表，损坏时记 warning 并回退空列表。
    pub fn load(&self) -> PanelResult<Vec<Subscription>> {
        let path = self.file();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let text = std::fs::read_to_string(&path)?;
        match serde_json::from_str(&text) {
            Ok(subs) => Ok(subs),
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "subscriptions.json unreadable, fall back to empty list"
                );
                Ok(Vec::new())
            }
        }
    }

    /// 保存订阅列表到 `data_dir/subscriptions.json`。
    pub fn save(&self, subs: &[Subscription]) -> PanelResult<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        let text = serde_json::to_string_pretty(subs)?;
        std::fs::write(self.file(), text)?;
        Ok(())
    }

    /// 追加一条订阅并落盘。
    pub fn add(&self, name: &str, url: &str, enabled: bool) -> PanelResult<Subscription> {
        let mut subs = self.load()?;
        let sub = Subscription {
            id: Uuid::new_v4(),
            name: name.to_string(),
            url: url.to_string(),
            enabled,
            userinfo: None,
            node_count: 0,
            error: None,
        };
        subs.push(sub.clone());
        self.save(&subs)?;
        Ok(sub)
    }

    /// 按 id 移除订阅并落盘；不存在时静默返回。
    pub fn remove(&self, id: Uuid) -> PanelResult<()> {
        let mut subs = self.load()?;
        let before = subs.len();
        subs.retain(|s| s.id != id);
        if subs.len() == before {
            return Ok(());
        }
        self.save(&subs)
    }

    /// 按 id 切换订阅启用状态并落盘；不存在时静默返回。
    pub fn set_enabled(&self, id: Uuid, enabled: bool) -> PanelResult<()> {
        let mut subs = self.load()?;
        let mut found = false;
        for sub in &mut subs {
            if sub.id == id {
                sub.enabled = enabled;
                found = true;
            }
        }
        if !found {
            return Ok(());
        }
        self.save(&subs)
    }
}

/// 订阅内容格式（嗅探结果）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubFormat {
    /// 分享链接（base64 或明文行列表）。
    ShareLinks,
    /// clash/mihomo YAML（含 `proxies:`）。
    ClashYaml,
    /// sing-box JSON（含 `outbounds`）。
    SingBoxJson,
}

/// 订阅拉取结果：嗅探出的格式 + 双核心节点 + 用户信息 + 行级 warning。
#[derive(Debug, Clone)]
pub struct FetchResult {
    pub format: SubFormat,
    pub singbox_nodes: Vec<Value>,
    pub mihomo_nodes: Vec<Value>,
    pub userinfo: Option<SubscriptionInfo>,
    pub warnings: Vec<String>,
}

/// 订阅拉取器。
#[derive(Debug, Clone)]
pub struct SubscriptionFetcher {
    client: reqwest::Client,
}

impl Default for SubscriptionFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl SubscriptionFetcher {
    /// 创建一个默认的订阅拉取器（15 秒请求超时，禁用系统代理）。
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .no_proxy()
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { client }
    }

    /// 使用自定义 HTTP 客户端创建拉取器。
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// 拉取 sing-box 订阅配置，返回配置 JSON 与可选的用户信息。
    ///
    /// 旧版 Hub 路径，保留用于兼容（新路径见 [`fetch_subscription`]）。
    pub async fn fetch_singbox_config(
        &self,
        hub_url: &str,
        token: &str,
    ) -> PanelResult<(Value, Option<SubscriptionInfo>)> {
        let url = format!(
            "{}/sub/{}?format=singbox",
            hub_url.trim_end_matches('/'),
            token
        );
        tracing::debug!(url = %url, "fetching subscription sing-box config");

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| PanelError::Client(format!("subscription request failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            return Err(PanelError::Client(format!(
                "subscription request returned HTTP {status}"
            )));
        }

        let info = parse_subscription_userinfo(resp.headers());
        let text = resp
            .text()
            .await
            .map_err(|e| PanelError::Client(format!("failed to read subscription body: {e}")))?;
        let config: Value = serde_json::from_str(&text).map_err(|e| {
            PanelError::Client(format!("invalid sing-box config in subscription: {e}"))
        })?;

        Ok((config, info))
    }

    /// 拉取 clash/mihomo 订阅配置，返回 YAML 原文与可选的用户信息。
    ///
    /// 旧版 Hub 路径，保留用于兼容（新路径见 [`fetch_subscription`]）。
    pub async fn fetch_clash_config(
        &self,
        hub_url: &str,
        token: &str,
    ) -> PanelResult<(String, Option<SubscriptionInfo>)> {
        let url = format!(
            "{}/sub/{}?format=clash",
            hub_url.trim_end_matches('/'),
            token
        );
        tracing::debug!(url = %url, "fetching subscription clash config");

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| PanelError::Client(format!("subscription request failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            return Err(PanelError::Client(format!(
                "subscription request returned HTTP {status}"
            )));
        }

        let info = parse_subscription_userinfo(resp.headers());
        let text = resp
            .text()
            .await
            .map_err(|e| PanelError::Client(format!("failed to read subscription body: {e}")))?;

        Ok((text, info))
    }

    /// 通用订阅拉取：任意 URL，自动嗅探格式（见 [`fetch_subscription`]）。
    pub async fn fetch(&self, url: &str) -> PanelResult<FetchResult> {
        tracing::debug!(url = %url, "fetching subscription");
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| PanelError::Client(format!("subscription request failed: {e}")))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(PanelError::Client(format!(
                "subscription request returned HTTP {status}"
            )));
        }
        let info = parse_subscription_userinfo(resp.headers());
        let text = resp
            .text()
            .await
            .map_err(|e| PanelError::Client(format!("failed to read subscription body: {e}")))?;
        parse_subscription_body(&text, info)
    }
}

/// 通用订阅拉取（模块级入口）：GET（no_proxy、30s 超时、UA "clash.meta"）→ 嗅探格式。
pub async fn fetch_subscription(url: &str) -> PanelResult<FetchResult> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .no_proxy()
        .user_agent("clash.meta")
        .build()
        .map_err(|e| PanelError::Client(format!("failed to build http client: {e}")))?;
    SubscriptionFetcher::with_client(client).fetch(url).await
}

/// 解析 `subscription-userinfo` 响应头（`upload=..; download=..; total=..; expire=..`）。
pub fn parse_subscription_userinfo(
    headers: &reqwest::header::HeaderMap,
) -> Option<SubscriptionInfo> {
    let value = headers.get("subscription-userinfo")?.to_str().ok()?;
    let mut info = SubscriptionInfo::default();
    for pair in value.split(';') {
        let Some((key, val)) = pair.split_once('=') else {
            continue;
        };
        match (key.trim(), val.trim()) {
            ("upload", v) => info.upload = v.parse().ok(),
            ("download", v) => info.download = v.parse().ok(),
            ("total", v) => info.total = v.parse().ok(),
            ("expire", v) => info.expire = v.parse().ok(),
            _ => {}
        }
    }
    Some(info)
}

/// 嗅探订阅内容格式并转换为双核心节点：
///
/// ① 整体 base64 解码成功且含 `://` → 分享链接
/// ② 含 `proxies:` → clash YAML（proxies 转双格式）
/// ③ JSON 含 `outbounds` → sing-box JSON（extract 后转 mihomo）
/// ④ 按行含 `://` → 明文分享链接
pub fn parse_subscription_body(
    text: &str,
    info: Option<SubscriptionInfo>,
) -> PanelResult<FetchResult> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(PanelError::Client("subscription body is empty".to_string()));
    }

    // ① base64 分享链接列表。
    if let Some(decoded) = crate::share_link::b64_decode(trimmed) {
        if decoded.contains("://") {
            return Ok(share_links_result(
                parse_share_links(&decoded),
                info,
                SubFormat::ShareLinks,
            ));
        }
    }

    // ② clash YAML。
    if trimmed.contains("proxies:") {
        let proxies = extract_nodes_mihomo(trimmed)?;
        let mut singbox_nodes = Vec::with_capacity(proxies.len());
        let mut warnings = Vec::new();
        for p in &proxies {
            match mihomo_to_singbox(p) {
                Some(o) => singbox_nodes.push(o),
                None => {
                    if let Some(n) = p.get("name").and_then(Value::as_str) {
                        warnings.push(format!("unsupported clash proxy type skipped: {n}"));
                    }
                }
            }
        }
        return Ok(FetchResult {
            format: SubFormat::ClashYaml,
            singbox_nodes,
            mihomo_nodes: proxies,
            userinfo: info,
            warnings,
        });
    }

    // ③ sing-box JSON。
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
            if parsed.get("outbounds").and_then(Value::as_array).is_some() {
                let outbounds = extract_nodes_singbox(&parsed);
                let mut mihomo_nodes = Vec::with_capacity(outbounds.len());
                let mut warnings = Vec::new();
                for o in &outbounds {
                    match singbox_to_mihomo(o) {
                        Some(p) => mihomo_nodes.push(p),
                        None => {
                            if let Some(t) = o.get("tag").and_then(Value::as_str) {
                                warnings.push(format!(
                                    "unsupported sing-box outbound type skipped: {t}"
                                ));
                            }
                        }
                    }
                }
                return Ok(FetchResult {
                    format: SubFormat::SingBoxJson,
                    singbox_nodes: outbounds,
                    mihomo_nodes,
                    userinfo: info,
                    warnings,
                });
            }
        }
    }

    // ④ 明文分享链接。
    if trimmed.lines().any(|l| l.contains("://")) {
        return Ok(share_links_result(
            parse_share_links(trimmed),
            info,
            SubFormat::ShareLinks,
        ));
    }

    Err(PanelError::Client(
        "unrecognized subscription format".to_string(),
    ))
}

/// 分享链接解析结果 → [`FetchResult`]。
fn share_links_result(
    parsed: ShareLinkParseResult,
    info: Option<SubscriptionInfo>,
    format: SubFormat,
) -> FetchResult {
    let mut singbox_nodes = Vec::with_capacity(parsed.nodes.len());
    let mut mihomo_nodes = Vec::with_capacity(parsed.nodes.len());
    for node in &parsed.nodes {
        singbox_nodes.push(node.outbound_singbox.clone());
        mihomo_nodes.push(node.proxy_mihomo.clone());
    }
    FetchResult {
        format,
        singbox_nodes,
        mihomo_nodes,
        userinfo: info,
        warnings: parsed.warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine as _;

    const SUB_JSON: &str = r#"{
        "log": { "level": "info" },
        "outbounds": [
            { "type": "vless", "tag": "n1", "server": "example.com", "server_port": 443,
              "uuid": "12345678-1234-1234-1234-123456789012",
              "tls": { "enabled": true, "server_name": "example.com" } },
            { "type": "hysteria2", "tag": "n2", "server": "example.org", "server_port": 8443,
              "password": "pw", "tls": { "enabled": true, "server_name": "example.org" } },
            { "type": "selector", "tag": "proxy", "outbounds": ["n1"] },
            { "type": "direct", "tag": "direct" }
        ],
        "route": { "final": "n1" }
    }"#;

    const SUB_YAML: &str = "port: 7890\nproxies:\n  - name: n1\n    type: vless\n    server: example.com\n    port: 443\n    uuid: 12345678-1234-1234-1234-123456789012\n  - name: n2\n    type: ss\n    server: example.org\n    port: 8388\n    cipher: aes-256-gcm\n    password: pw\nrules:\n  - MATCH,DIRECT\n";

    const SHARE_LINKS: &str = "ss://YWVzLTI1Ni1nY206cGFzc3dvcmQ@example.com:8388#ss-node\nvless://12345678-1234-1234-1234-123456789012@example.com:443?security=tls&sni=example.com#vless-node\n";

    fn b64(s: &str) -> String {
        base64::engine::general_purpose::STANDARD.encode(s)
    }

    async fn spawn_server(app: axum::Router) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    // ---------- 旧版 Hub 路径回归（⑥） ----------

    #[tokio::test]
    async fn fetch_singbox_config_parses_config_and_info() {
        let app = axum::Router::new().route(
            "/sub/{token}",
            axum::routing::get(|| async {
                (
                    [(
                        "subscription-userinfo",
                        "upload=100; download=200; total=1000; expire=1700000000",
                    )],
                    SUB_JSON,
                )
            }),
        );
        let base = spawn_server(app).await;

        let fetcher = SubscriptionFetcher::new();
        let (config, info) = fetcher.fetch_singbox_config(&base, "tok").await.unwrap();

        assert_eq!(config["outbounds"][0]["tag"], "n1");
        assert_eq!(config["route"]["final"], "n1");

        let info = info.unwrap();
        assert_eq!(info.upload, Some(100));
        assert_eq!(info.download, Some(200));
        assert_eq!(info.total, Some(1000));
        assert_eq!(info.expire, Some(1700000000));
    }

    #[tokio::test]
    async fn fetch_singbox_config_returns_client_error_on_4xx() {
        let app = axum::Router::new().route(
            "/sub/{token}",
            axum::routing::get(|| async { axum::http::StatusCode::NOT_FOUND }),
        );
        let base = spawn_server(app).await;

        let fetcher = SubscriptionFetcher::new();
        let err = fetcher
            .fetch_singbox_config(&base, "missing")
            .await
            .unwrap_err();
        assert!(matches!(err, PanelError::Client(_)));
    }

    #[tokio::test]
    async fn fetch_clash_config_returns_yaml_text_and_info() {
        let app = axum::Router::new().route(
            "/sub/{token}",
            axum::routing::get(|| async {
                (
                    [(
                        "subscription-userinfo",
                        "upload=100; download=200; total=1000; expire=1700000000",
                    )],
                    SUB_YAML,
                )
            }),
        );
        let base = spawn_server(app).await;

        let fetcher = SubscriptionFetcher::new();
        let (yaml, info) = fetcher.fetch_clash_config(&base, "tok").await.unwrap();

        // YAML 原文原样返回。
        assert_eq!(yaml, SUB_YAML);
        assert!(yaml.contains("proxies:"));

        let info = info.unwrap();
        assert_eq!(info.upload, Some(100));
        assert_eq!(info.download, Some(200));
        assert_eq!(info.total, Some(1000));
        assert_eq!(info.expire, Some(1700000000));
    }

    #[tokio::test]
    async fn fetch_clash_config_returns_client_error_on_4xx() {
        let app = axum::Router::new().route(
            "/sub/{token}",
            axum::routing::get(|| async { axum::http::StatusCode::NOT_FOUND }),
        );
        let base = spawn_server(app).await;

        let fetcher = SubscriptionFetcher::new();
        let err = fetcher
            .fetch_clash_config(&base, "missing")
            .await
            .unwrap_err();
        assert!(matches!(err, PanelError::Client(_)));
    }

    #[test]
    fn parse_userinfo_ignores_malformed_pairs() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::HeaderName::from_static("subscription-userinfo"),
            reqwest::header::HeaderValue::from_static("upload=1; bad; total=1024; expire=5"),
        );
        let info = parse_subscription_userinfo(&headers).unwrap();
        assert_eq!(info.upload, Some(1));
        assert_eq!(info.total, Some(1024));
        assert_eq!(info.expire, Some(5));
        assert_eq!(info.download, None);
    }

    // ---------- ② 四种格式嗅探各 1 例 ----------

    #[test]
    fn sniff_base64_share_links() {
        let body = b64(SHARE_LINKS);
        let result = parse_subscription_body(&body, None).unwrap();
        assert_eq!(result.format, SubFormat::ShareLinks);
        assert_eq!(result.singbox_nodes.len(), 2);
        assert_eq!(result.mihomo_nodes.len(), 2);
        assert_eq!(result.singbox_nodes[0]["type"], "shadowsocks");
        assert_eq!(result.singbox_nodes[1]["type"], "vless");
    }

    #[test]
    fn sniff_clash_yaml() {
        let result = parse_subscription_body(SUB_YAML, None).unwrap();
        assert_eq!(result.format, SubFormat::ClashYaml);
        assert_eq!(result.mihomo_nodes.len(), 2);
        assert_eq!(result.mihomo_nodes[0]["type"], "vless");
        assert_eq!(result.singbox_nodes.len(), 2);
        assert_eq!(result.singbox_nodes[0]["type"], "vless");
        assert_eq!(result.singbox_nodes[1]["type"], "shadowsocks");
        assert_eq!(result.singbox_nodes[1]["method"], "aes-256-gcm");
    }

    #[test]
    fn sniff_singbox_json() {
        let result = parse_subscription_body(SUB_JSON, None).unwrap();
        assert_eq!(result.format, SubFormat::SingBoxJson);
        assert_eq!(result.singbox_nodes.len(), 2);
        assert_eq!(result.singbox_nodes[0]["tag"], "n1");
        assert_eq!(result.mihomo_nodes.len(), 2);
        assert_eq!(result.mihomo_nodes[0]["name"], "n1");
        assert_eq!(result.mihomo_nodes[0]["type"], "vless");
    }

    #[test]
    fn sniff_plaintext_share_links() {
        let result = parse_subscription_body(SHARE_LINKS, None).unwrap();
        assert_eq!(result.format, SubFormat::ShareLinks);
        assert_eq!(result.singbox_nodes.len(), 2);
        assert_eq!(result.mihomo_nodes.len(), 2);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn sniff_rejects_unrecognized_content() {
        let err = parse_subscription_body("just some plain text", None).unwrap_err();
        assert!(matches!(err, PanelError::Client(_)));
        let err = parse_subscription_body("  ", None).unwrap_err();
        assert!(matches!(err, PanelError::Client(_)));
    }

    #[test]
    fn sniff_unsupported_clash_proxy_is_skipped_with_warning() {
        let yaml = "proxies:\n  - name: ok\n    type: vless\n    server: a.com\n    port: 443\n    uuid: 12345678-1234-1234-1234-123456789012\n  - name: bad\n    type: wireguard\n    server: b.com\n    port: 51820\n";
        let result = parse_subscription_body(yaml, None).unwrap();
        assert_eq!(result.mihomo_nodes.len(), 2, "mihomo 侧保留原 proxies");
        assert_eq!(result.singbox_nodes.len(), 1, "sing-box 侧跳过不支持类型");
        assert_eq!(result.singbox_nodes[0]["tag"], "ok");
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("bad"));
    }

    // ---------- ③ fetch_subscription e2e（本地 axum，禁外部网络） ----------

    #[tokio::test]
    async fn fetch_subscription_gets_base64_links_and_userinfo() {
        let body = b64(SHARE_LINKS);
        let app = axum::Router::new().route(
            "/sub",
            axum::routing::get(move || async move {
                (
                    [(
                        "subscription-userinfo",
                        "upload=10; download=20; total=1000; expire=1700000000",
                    )],
                    body,
                )
            }),
        );
        let base = spawn_server(app).await;

        let result = fetch_subscription(&format!("{base}/sub")).await.unwrap();
        assert_eq!(result.format, SubFormat::ShareLinks);
        assert_eq!(result.singbox_nodes.len(), 2);
        let info = result.userinfo.unwrap();
        assert_eq!(info.upload, Some(10));
        assert_eq!(info.download, Some(20));
    }

    #[tokio::test]
    async fn fetch_subscription_returns_client_error_on_4xx() {
        let app = axum::Router::new().route(
            "/sub",
            axum::routing::get(|| async { axum::http::StatusCode::NOT_FOUND }),
        );
        let base = spawn_server(app).await;
        let err = fetch_subscription(&format!("{base}/sub"))
            .await
            .unwrap_err();
        assert!(matches!(err, PanelError::Client(_)));
    }

    // ---------- ④ SubscriptionStore CRUD ----------

    #[test]
    fn subscription_store_crud() {
        let dir = tempfile::tempdir().unwrap();
        let store = SubscriptionStore::new(dir.path().to_path_buf());

        // 缺失文件 → 空列表。
        assert!(store.load().unwrap().is_empty());
        assert!(!store.file().exists());

        // add。
        let sub1 = store.add("sub-a", "https://example.com/sub", true).unwrap();
        let sub2 = store
            .add("sub-b", "https://example.org/sub", false)
            .unwrap();
        let mut all = store.load().unwrap();
        assert_eq!(all.len(), 2);
        assert_ne!(sub1.id, sub2.id);

        // set_enabled。
        store.set_enabled(sub1.id, false).unwrap();
        all = store.load().unwrap();
        assert!(!all.iter().find(|s| s.id == sub1.id).unwrap().enabled);

        // remove。
        store.remove(sub1.id).unwrap();
        all = store.load().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, sub2.id);

        // 对不存在的 id 静默。
        store.remove(Uuid::new_v4()).unwrap();
        store.set_enabled(Uuid::new_v4(), true).unwrap();
        assert_eq!(store.load().unwrap().len(), 1);
    }

    #[test]
    fn subscription_store_tolerates_corrupted_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = SubscriptionStore::new(dir.path().to_path_buf());
        std::fs::write(store.file(), "{ not json").unwrap();
        assert!(store.load().unwrap().is_empty());
    }
}
