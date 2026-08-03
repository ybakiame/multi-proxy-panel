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
    /// 拉取时使用的请求 User-Agent；`None` / 空串使用默认 `clash.meta`。
    ///
    /// 部分订阅源按 UA 返回不同格式，可自定义复写（`#[serde(default)]` 保证旧版
    /// `subscriptions.json`（无此字段）可正常反序列化）。
    #[serde(default)]
    pub user_agent: Option<String>,
    /// 最近一次 fetch 嗅探出的订阅内容格式（`ShareLinks` / `ClashYaml` /
    /// `SingBoxJson`）；尚未成功拉取时为 `None`。
    ///
    /// `#[serde(default)]` 保证旧版 `subscriptions.json`（无此字段）可正常反序列化。
    #[serde(default)]
    pub format: Option<SubFormat>,
    /// 关联的覆写模板（`data_dir/profiles.json` 中的模板 id）；`None` = 不使用覆写。
    ///
    /// 纯关联制：运行时使用的覆写 = 当前选中订阅关联的模板。`#[serde(default)]`
    /// 保证旧版 `subscriptions.json`（无此字段）可正常反序列化。
    #[serde(default)]
    pub profile_id: Option<Uuid>,
}

/// 订阅存储：读写 `data_dir/subscriptions.json`（load / save / add / remove /
/// set_enabled / set_profile_id）。
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
    pub fn add(
        &self,
        name: &str,
        url: &str,
        enabled: bool,
        user_agent: Option<&str>,
    ) -> PanelResult<Subscription> {
        let mut subs = self.load()?;
        let sub = Subscription {
            id: Uuid::new_v4(),
            name: name.to_string(),
            url: url.to_string(),
            enabled,
            userinfo: None,
            node_count: 0,
            error: None,
            user_agent: user_agent.map(str::to_string),
            format: None,
            profile_id: None,
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

    /// 按 id 设置订阅关联的覆写模板（`None` = 取消关联）并落盘；订阅不存在时报错。
    pub fn set_profile_id(&self, id: Uuid, profile_id: Option<Uuid>) -> PanelResult<()> {
        let mut subs = self.load()?;
        let target = subs
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| PanelError::Client(format!("订阅不存在（id: {id}）")))?;
        target.profile_id = profile_id;
        self.save(&subs)
    }

    /// 按 id 更新订阅的 name / url / user_agent 并落盘；订阅不存在时报错。
    ///
    /// URL 变更时清空上次 fetch 的缓存（`userinfo` / `node_count`），避免旧数据
    /// 在新 URL 下误导展示；URL 未变则保留缓存。
    pub fn update(
        &self,
        id: Uuid,
        name: &str,
        url: &str,
        user_agent: Option<&str>,
    ) -> PanelResult<()> {
        let mut subs = self.load()?;
        let target = subs
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| PanelError::Client(format!("订阅不存在（id: {id}）")))?;
        let url_changed = target.url != url;
        target.name = name.to_string();
        target.url = url.to_string();
        target.user_agent = user_agent.map(str::to_string);
        if url_changed {
            target.userinfo = None;
            target.node_count = 0;
            target.format = None;
            // URL 变更意味着旧 URL 拉取的内容缓存也失效，一并删除。
            self.clear_cached_content(id);
        }
        self.save(&subs)
    }

    /// `data_dir/subscription_cache/<id>.json`。
    pub fn cache_file(&self, id: Uuid) -> PathBuf {
        self.data_dir
            .join("subscription_cache")
            .join(format!("{id}.json"))
    }

    /// 写入订阅内容缓存到 `data_dir/subscription_cache/<id>.json`。
    pub fn write_cached_content(
        &self,
        id: Uuid,
        content: &CachedSubscriptionContent,
    ) -> PanelResult<()> {
        let dir = self.data_dir.join("subscription_cache");
        std::fs::create_dir_all(&dir)?;
        let text = serde_json::to_string_pretty(content)?;
        std::fs::write(dir.join(format!("{id}.json")), text)?;
        Ok(())
    }

    /// 读取订阅内容缓存；文件缺失 / 不可读 / 损坏时返回 `None`（记 debug / warn
    /// 日志，不向调用方报错，由调用方回退远程拉取）。
    pub fn load_cached_content(&self, id: Uuid) -> Option<CachedSubscriptionContent> {
        let path = self.cache_file(id);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::debug!(path = %path.display(), "subscription cache missing");
                return None;
            }
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "subscription cache unreadable, ignore"
                );
                return None;
            }
        };
        match serde_json::from_str(&text) {
            Ok(content) => Some(content),
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "subscription cache corrupted, ignore"
                );
                None
            }
        }
    }

    /// 删除订阅内容缓存文件（URL 变更等场景）；文件不存在时静默。
    pub fn clear_cached_content(&self, id: Uuid) {
        let path = self.cache_file(id);
        match std::fs::remove_file(&path) {
            Ok(()) => tracing::debug!(path = %path.display(), "subscription cache cleared"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => tracing::warn!(
                path = %path.display(),
                error = %e,
                "failed to clear subscription cache"
            ),
        }
    }
}

/// 订阅内容格式（嗅探结果）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubFormat {
    /// 分享链接（base64 或明文行列表）。
    ShareLinks,
    /// clash/mihomo YAML（含 `proxies:`）。
    ClashYaml,
    /// sing-box JSON（含 `outbounds`）。
    SingBoxJson,
}

impl Default for SubFormat {
    /// 缺省格式取分享链接（双核心皆可，作为旧缓存缺 `format` 字段时的兼容回退）。
    fn default() -> Self {
        Self::ShareLinks
    }
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

/// 订阅内容缓存：刷新成功时落盘（`data_dir/subscription_cache/<id>.json`）的
/// 双核心节点 + 嗅探格式。
///
/// 供配置预览等场景优先本地组装、避免每次远程拉取。所有字段带
/// `#[serde(default)]`，保证字段缺失的旧缓存文件可兼容反序列化。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedSubscriptionContent {
    /// 嗅探出的订阅格式（与 [`Subscription::format`] 一致）。
    #[serde(default)]
    pub format: SubFormat,
    /// sing-box 侧可用节点（`outbounds` 数组元素）。
    #[serde(default)]
    pub singbox_nodes: Vec<Value>,
    /// mihomo 侧节点（`proxies` 数组元素）。
    #[serde(default)]
    pub mihomo_nodes: Vec<Value>,
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
    ///
    /// GitHub blob/raw 链接在进入请求前归一化为 `raw.githubusercontent.com`。
    pub async fn fetch(&self, url: &str) -> PanelResult<FetchResult> {
        let url = crate::normalize_resource_url(url);
        tracing::debug!(url = %url, "fetching subscription");
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
        parse_subscription_body(&text, info)
    }
}

/// 通用订阅拉取（模块级入口）：GET（no_proxy、30s 超时）→ 嗅探格式。
///
/// `ua` 为 `None` / 空串时使用默认 UA `clash.meta`；部分订阅源按 UA 返回不同
/// 格式，可传入自定义 UA 复写。
pub async fn fetch_subscription_with_ua(url: &str, ua: Option<&str>) -> PanelResult<FetchResult> {
    let ua = ua
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("clash.meta");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .no_proxy()
        .user_agent(ua)
        .build()
        .map_err(|e| PanelError::Client(format!("failed to build http client: {e}")))?;
    SubscriptionFetcher::with_client(client).fetch(url).await
}

/// 通用订阅拉取（模块级入口）：GET（no_proxy、30s 超时、默认 UA "clash.meta"）→ 嗅探格式。
pub async fn fetch_subscription(url: &str) -> PanelResult<FetchResult> {
    fetch_subscription_with_ua(url, None).await
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

    // ---------- ③.1 UA 复写：自定义 UA 与默认 clash.meta ----------

    #[tokio::test]
    async fn fetch_subscription_sends_custom_or_default_user_agent() {
        let body = b64(SHARE_LINKS);
        let uas = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let app = {
            let uas = std::sync::Arc::clone(&uas);
            axum::Router::new().route(
                "/sub",
                axum::routing::get(move |headers: axum::http::HeaderMap| {
                    let uas = std::sync::Arc::clone(&uas);
                    async move {
                        uas.lock().unwrap().push(
                            headers
                                .get("user-agent")
                                .and_then(|v| v.to_str().ok())
                                .unwrap_or_default()
                                .to_string(),
                        );
                        body.clone()
                    }
                }),
            )
        };
        let base = spawn_server(app).await;

        // 自定义 UA 透传。
        let result = fetch_subscription_with_ua(&format!("{base}/sub"), Some("clash-verge/0.6.5"))
            .await
            .unwrap();
        assert_eq!(result.format, SubFormat::ShareLinks);
        assert_eq!(result.singbox_nodes.len(), 2);

        // 缺省（None）与空串均回退默认 clash.meta。
        let result = fetch_subscription(&format!("{base}/sub")).await.unwrap();
        assert_eq!(result.format, SubFormat::ShareLinks);
        let result = fetch_subscription_with_ua(&format!("{base}/sub"), Some("  "))
            .await
            .unwrap();
        assert_eq!(result.format, SubFormat::ShareLinks);

        let got = uas.lock().unwrap();
        assert_eq!(
            got.as_slice(),
            ["clash-verge/0.6.5", "clash.meta", "clash.meta"]
        );
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
        let sub1 = store
            .add("sub-a", "https://example.com/sub", true, None)
            .unwrap();
        let sub2 = store
            .add(
                "sub-b",
                "https://example.org/sub",
                false,
                Some("clash-verge"),
            )
            .unwrap();
        let mut all = store.load().unwrap();
        assert_eq!(all.len(), 2);
        assert_ne!(sub1.id, sub2.id);
        assert!(
            all.iter().all(|s| s.profile_id.is_none()),
            "新订阅默认不关联覆写"
        );

        // set_enabled。
        store.set_enabled(sub1.id, false).unwrap();
        all = store.load().unwrap();
        assert!(!all.iter().find(|s| s.id == sub1.id).unwrap().enabled);

        // set_profile_id：关联覆写模板并落盘；None 取消关联；不存在 id 报错。
        let profile_id = Uuid::new_v4();
        store.set_profile_id(sub1.id, Some(profile_id)).unwrap();
        all = store.load().unwrap();
        assert_eq!(
            all.iter().find(|s| s.id == sub1.id).unwrap().profile_id,
            Some(profile_id)
        );
        store.set_profile_id(sub1.id, None).unwrap();
        all = store.load().unwrap();
        assert_eq!(
            all.iter().find(|s| s.id == sub1.id).unwrap().profile_id,
            None
        );
        assert!(
            store
                .set_profile_id(Uuid::new_v4(), Some(profile_id))
                .is_err()
        );

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

    /// UA 持久化 + 旧版 subscriptions.json（无 user_agent 字段）兼容。
    #[test]
    fn subscription_store_persists_user_agent_and_tolerates_legacy() {
        let dir = tempfile::tempdir().unwrap();
        let store = SubscriptionStore::new(dir.path().to_path_buf());

        let sub = store
            .add("sub", "https://example.com/sub", true, Some("sing-box"))
            .unwrap();
        assert_eq!(sub.user_agent.as_deref(), Some("sing-box"));
        let loaded = store.load().unwrap();
        assert_eq!(loaded[0].user_agent.as_deref(), Some("sing-box"));

        // 旧版文件无 user_agent / profile_id 字段 → 反序列化为 None。
        std::fs::write(
            store.file(),
            r#"[{"id":"00000000-0000-0000-0000-000000000001","name":"old","url":"https://x.com/sub","enabled":true,"node_count":0}]"#,
        )
        .unwrap();
        let legacy = store.load().unwrap();
        assert_eq!(legacy[0].user_agent, None);
        assert_eq!(legacy[0].profile_id, None);
    }

    /// ④.1 update：name / url / user_agent 更新落盘；URL 变更清空 userinfo 缓存，
    /// URL 未变保留缓存。
    #[test]
    fn subscription_store_update_changes_fields_and_clears_userinfo_on_url_change() {
        let dir = tempfile::tempdir().unwrap();
        let store = SubscriptionStore::new(dir.path().to_path_buf());
        let sub = store
            .add("sub", "https://example.com/a", true, Some("ua"))
            .unwrap();
        // 关联一个覆写模板（update 不应清掉该关联）。
        let profile_id = Uuid::new_v4();
        store.set_profile_id(sub.id, Some(profile_id)).unwrap();
        // 模拟一次成功 fetch：写入 userinfo 与 node_count。
        let mut subs = store.load().unwrap();
        let s = subs.iter_mut().find(|s| s.id == sub.id).unwrap();
        s.userinfo = Some(SubscriptionInfo {
            upload: Some(1),
            download: Some(2),
            total: Some(100),
            expire: None,
        });
        s.node_count = 5;
        store.save(&subs).unwrap();

        // URL 变更 → userinfo / node_count 清空，name / user_agent 更新。
        store
            .update(sub.id, "new-name", "https://example.com/b", Some("new-ua"))
            .unwrap();
        let subs = store.load().unwrap();
        let s = subs.iter().find(|s| s.id == sub.id).unwrap();
        assert_eq!(s.name, "new-name");
        assert_eq!(s.url, "https://example.com/b");
        assert_eq!(s.user_agent.as_deref(), Some("new-ua"));
        assert_eq!(s.userinfo, None);
        assert_eq!(s.node_count, 0);
        assert_eq!(s.profile_id, Some(profile_id), "update 不改订阅关联");

        // URL 未变 → 缓存保留（仅改名称）。
        store
            .update(sub.id, "renamed", "https://example.com/b", None)
            .unwrap();
        let subs = store.load().unwrap();
        let s = subs.iter().find(|s| s.id == sub.id).unwrap();
        assert_eq!(s.name, "renamed");
        assert_eq!(s.user_agent, None);
        assert_eq!(s.node_count, 0);
    }

    /// ④.2 update：不存在的 id 报错，不落盘。
    #[test]
    fn subscription_store_update_returns_error_for_missing_id() {
        let dir = tempfile::tempdir().unwrap();
        let store = SubscriptionStore::new(dir.path().to_path_buf());
        let err = store
            .update(Uuid::new_v4(), "x", "https://example.com", None)
            .unwrap_err();
        assert!(err.to_string().contains("不存在"));
        assert!(store.load().unwrap().is_empty());
    }

    // ---------- ⑤ 订阅内容缓存（data_dir/subscription_cache/<id>.json） ----------

    /// 写读往返：未写入 → None；写入后原样读回；文件路径符合约定；字段缺失的
    /// 旧缓存（`{}`）反序列化为默认值。
    #[test]
    fn subscription_cache_write_read_roundtrip_and_legacy() {
        let dir = tempfile::tempdir().unwrap();
        let store = SubscriptionStore::new(dir.path().to_path_buf());
        let sub = store
            .add("sub", "https://example.com/sub", true, None)
            .unwrap();

        // 未写入 → None。
        assert!(store.load_cached_content(sub.id).is_none());

        let cached = CachedSubscriptionContent {
            format: SubFormat::ShareLinks,
            singbox_nodes: vec![serde_json::json!({ "tag": "n1", "type": "vless" })],
            mihomo_nodes: vec![serde_json::json!({ "name": "n1", "type": "vless" })],
        };
        store.write_cached_content(sub.id, &cached).unwrap();
        assert_eq!(store.load_cached_content(sub.id), Some(cached));

        // 文件路径符合约定：data_dir/subscription_cache/<id>.json。
        let path = store.cache_file(sub.id);
        assert!(path.starts_with(dir.path()));
        assert!(path.ends_with(format!("{}.json", sub.id)));
        assert!(path.exists());

        // 字段缺失的旧缓存 → `#[serde(default)]` 兼容，不报错。
        let dir2 = tempfile::tempdir().unwrap();
        let store2 = SubscriptionStore::new(dir2.path().to_path_buf());
        let sub2 = store2
            .add("sub2", "https://example.com/sub2", true, None)
            .unwrap();
        let legacy_path = store2.cache_file(sub2.id);
        std::fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
        std::fs::write(&legacy_path, "{}").unwrap();
        let legacy = store2.load_cached_content(sub2.id).unwrap();
        assert_eq!(legacy.format, SubFormat::ShareLinks);
        assert!(legacy.singbox_nodes.is_empty());
        assert!(legacy.mihomo_nodes.is_empty());
    }

    /// 损坏的缓存文件 → `None`（记 warn，不报错、不 panic）。
    #[test]
    fn subscription_cache_corrupted_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let store = SubscriptionStore::new(dir.path().to_path_buf());
        let sub = store
            .add("sub", "https://example.com/sub", true, None)
            .unwrap();
        let path = store.cache_file(sub.id);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{ not json").unwrap();
        assert!(store.load_cached_content(sub.id).is_none());
    }

    /// URL 变更 → 缓存文件删除；URL 未变（仅改名称）→ 缓存保留。
    #[test]
    fn subscription_cache_cleared_on_url_change() {
        let dir = tempfile::tempdir().unwrap();
        let store = SubscriptionStore::new(dir.path().to_path_buf());
        let sub = store
            .add("sub", "https://example.com/a", true, None)
            .unwrap();
        store
            .write_cached_content(
                sub.id,
                &CachedSubscriptionContent {
                    format: SubFormat::SingBoxJson,
                    singbox_nodes: vec![serde_json::json!({ "tag": "n1" })],
                    mihomo_nodes: Vec::new(),
                },
            )
            .unwrap();
        assert!(store.load_cached_content(sub.id).is_some());

        // URL 变更 → 缓存删除。
        store
            .update(sub.id, "sub", "https://example.com/b", None)
            .unwrap();
        assert!(store.load_cached_content(sub.id).is_none());

        // URL 未变（仅改名称）→ 缓存保留。
        store
            .write_cached_content(
                sub.id,
                &CachedSubscriptionContent {
                    format: SubFormat::SingBoxJson,
                    singbox_nodes: vec![serde_json::json!({ "tag": "n1" })],
                    mihomo_nodes: Vec::new(),
                },
            )
            .unwrap();
        store
            .update(sub.id, "renamed", "https://example.com/b", None)
            .unwrap();
        assert!(store.load_cached_content(sub.id).is_some());
    }
}
