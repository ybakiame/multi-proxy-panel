//! 脚本钩子引擎。
//!
//! 将 http-request / http-response 类型的脚本按 URL 规则挂载到 MITM 流量路径：
//! 请求/响应经过时命中规则的脚本依次执行，`$done` 返回值决定流量走向；脚本超时
//! 或抛异常时 no-op（透传原值）并记录警告。
//!
//! `$done` 语义（对齐 Surge / Loon / QX 三方交集）：
//!
//! | 返回值 | 行为 |
//! |--------|------|
//! | `$done()` / `$done({})` | 透传 |
//! | `$done({url})` | 替换请求 URL（请求阶段，不更新 Host 头） |
//! | `$done({status})` / `{statusCode}` | 改写响应状态码（QX 别名兼容） |
//! | `$done({headers})` | 整体替换 headers |
//! | `$done({body})` | 替换 body |
//! | `$done({response: {status, headers, body}})` | 脚本合成响应（请求阶段 mock） |
//! | `$done({abort: true})` | 中止请求（502 断开语义） |
//! | `$done("...")` | QX 风格：字符串直接作为新 body |

use std::sync::Arc;

use pp_script::{ScriptHost, ScriptKind, ScriptOutput, ScriptWorker};

/// 一条脚本钩子规则：URL 正则匹配 + 脚本源码。
pub struct ScriptRule {
    pub name: String,
    pub kind: pp_script::ScriptKind,
    pub pattern: regex::Regex,
    pub requires_body: bool,
    pub max_size: usize,
    pub source: String,
    /// Surge/Loon 模块 `argument=` 模板（`{key}` 占位已由调用方替换后的字符串）；
    /// `None` 表示脚本不声明模块参数（不注入 `$argument`）。
    pub argument: Option<String>,
}

/// 脚本钩子对流量的处置结果。
#[derive(Debug, Default, PartialEq, Eq)]
pub enum HookOutcome {
    /// 透传（url / headers / body 可能已被脚本就地改写）。
    #[default]
    Continue,
    /// `$done({response: {...}})`：脚本合成响应。
    Mock(MockResponse),
    /// `$done({abort: true})`：中止请求。
    Abort,
}

/// 脚本合成的响应（`$done({response: {...}})`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

/// 脚本钩子引擎：持有脚本执行 worker（收敛 `!Send` 的 QuickJS 执行）与规则列表。
pub struct ScriptHookEngine {
    worker: ScriptWorker,
    dialect: pp_script::ScriptDialect,
    rules: Vec<ScriptRule>,
}

/// 单条脚本执行期间的可变流量上下文。
struct HookCtx<'a> {
    url: Option<&'a mut String>,
    status: Option<&'a mut u16>,
    headers: &'a mut Vec<(String, String)>,
    body: &'a mut Option<String>,
}

impl ScriptHookEngine {
    /// 构造脚本钩子引擎（内部创建 [`ScriptWorker`]，对外 API 保持不变）。
    pub fn new(
        host: Arc<ScriptHost>,
        dialect: pp_script::ScriptDialect,
        limits: pp_script::ScriptLimits,
        rules: Vec<ScriptRule>,
    ) -> Self {
        let worker = ScriptWorker::new(host, limits);
        Self {
            worker,
            dialect,
            rules,
        }
    }

    /// 运行请求阶段钩子：命中规则且类型为 [`ScriptKind::HttpRequest`] 的脚本依次执行。
    ///
    /// 脚本 arg 为 `{url, method, path, headers, body}`（body 仅在 `requires_body`
    /// 且不超过 `max_size` 时注入）。返回首个非透传的处置结果（Mock/Abort 短路后续
    /// 脚本）。
    pub async fn run_request_hooks(
        &self,
        url: &mut String,
        method: &str,
        headers: &mut Vec<(String, String)>,
        body: &mut Option<String>,
    ) -> HookOutcome {
        let mut ctx = HookCtx {
            url: Some(url),
            status: None,
            headers,
            body,
        };
        for rule in self
            .rules
            .iter()
            .filter(|r| r.kind == ScriptKind::HttpRequest)
        {
            let cur_url = ctx
                .url
                .as_deref()
                .map(|s| s.to_string())
                .unwrap_or_default();
            if !rule.pattern.is_match(&cur_url) {
                continue;
            }
            let mut arg = serde_json::json!({
                "url": cur_url,
                "method": method,
                "path": url_path(&cur_url),
                "headers": headers_to_object(ctx.headers),
            });
            inject_body(&mut arg, rule, ctx.body);
            let outcome = self.run_one(rule, Some(arg), &mut ctx).await;
            if outcome != HookOutcome::Continue {
                return outcome;
            }
        }
        HookOutcome::Continue
    }

    /// 运行响应阶段钩子：命中规则且类型为 [`ScriptKind::HttpResponse`] 的脚本依次执行。
    ///
    /// 脚本 arg 为 `{status, statusCode, headers, body}`（`statusCode` 为 QX 别名；
    /// body 规则同请求阶段）。Mock 结果整体替换响应；Abort 语义由调用方处理。
    pub async fn run_response_hooks(
        &self,
        url: &str,
        status: &mut u16,
        headers: &mut Vec<(String, String)>,
        body: &mut Option<String>,
    ) -> HookOutcome {
        let mut ctx = HookCtx {
            url: None,
            status: Some(status),
            headers,
            body,
        };
        for rule in self
            .rules
            .iter()
            .filter(|r| r.kind == ScriptKind::HttpResponse)
        {
            if !rule.pattern.is_match(url) {
                continue;
            }
            let mut arg = serde_json::json!({
                "status": *ctx.status.as_deref().unwrap_or(&0),
                "statusCode": *ctx.status.as_deref().unwrap_or(&0),
                "headers": headers_to_object(ctx.headers),
            });
            inject_body(&mut arg, rule, ctx.body);
            let outcome = self.run_one(rule, Some(arg), &mut ctx).await;
            if outcome != HookOutcome::Continue {
                return outcome;
            }
        }
        HookOutcome::Continue
    }

    /// 执行单条规则：经 [`ScriptWorker`] 串行执行，成功时回写输出，失败/超时仅记录警告。
    async fn run_one(
        &self,
        rule: &ScriptRule,
        arg: Option<serde_json::Value>,
        ctx: &mut HookCtx<'_>,
    ) -> HookOutcome {
        match self
            .worker
            .run_script(
                &rule.source,
                rule.kind,
                arg,
                rule.argument.as_deref(),
                self.dialect,
                &rule.name,
            )
            .await
        {
            Ok(ScriptOutput(out)) => apply_output(&out, ctx),
            Err(e) => {
                tracing::warn!(script = %rule.name, "hook script failed: {e}");
                HookOutcome::Continue
            }
        }
    }
}

/// headers 列表 → JSON 对象（同名键后者覆盖前者）。
fn headers_to_object(headers: &[(String, String)]) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (k, v) in headers {
        map.insert(k.clone(), serde_json::Value::String(v.clone()));
    }
    serde_json::Value::Object(map)
}

/// 从绝对 URL 提取 path（含 query），供 QX 风格 `$request.path`。
fn url_path(url: &str) -> String {
    let after_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    match after_scheme.find('/') {
        Some(idx) => after_scheme[idx..].to_string(),
        None => "/".to_string(),
    }
}

/// 规则要求 body 且未超过 `max_size` 时，把 body 注入 arg。
fn inject_body(arg: &mut serde_json::Value, rule: &ScriptRule, body: &Option<String>) {
    if !rule.requires_body {
        return;
    }
    let Some(b) = body else { return };
    if b.len() > rule.max_size {
        return;
    }
    if let Some(obj) = arg.as_object_mut() {
        obj.insert("body".to_string(), serde_json::Value::String(b.clone()));
    }
}

/// 把 `$done` 返回值回写到流量上下文，并给出处置结果。
fn apply_output(out: &serde_json::Value, ctx: &mut HookCtx<'_>) -> HookOutcome {
    // QX 风格：$done("...") 字符串直接作为新 body。
    if let Some(s) = out.as_str() {
        *ctx.body = Some(s.to_string());
        return HookOutcome::Continue;
    }
    let Some(obj) = out.as_object() else {
        return HookOutcome::Continue;
    };
    if obj.get("abort").and_then(|v| v.as_bool()) == Some(true) {
        return HookOutcome::Abort;
    }
    if let Some(resp) = obj.get("response").and_then(|v| v.as_object()) {
        return HookOutcome::Mock(mock_from_json(resp));
    }
    if let Some(u) = obj.get("url").and_then(|v| v.as_str())
        && let Some(url) = ctx.url.as_deref_mut()
    {
        *url = u.to_string();
    }
    if let Some(status) = ctx.status.as_deref_mut() {
        let new_status = obj
            .get("status")
            .or_else(|| obj.get("statusCode"))
            .and_then(|v| v.as_u64())
            .and_then(|s| u16::try_from(s).ok());
        if let Some(s) = new_status {
            *status = s;
        }
    }
    if let Some(h) = obj.get("headers").and_then(|v| v.as_object()) {
        ctx.headers.clear();
        ctx.headers.extend(
            h.iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string())),
        );
    }
    if let Some(b) = obj.get("body").and_then(|v| v.as_str()) {
        *ctx.body = Some(b.to_string());
    }
    HookOutcome::Continue
}

/// `{response: {...}}` 对象 → [`MockResponse`]（status / statusCode 均可，缺省 200）。
fn mock_from_json(resp: &serde_json::Map<String, serde_json::Value>) -> MockResponse {
    let status = resp
        .get("status")
        .or_else(|| resp.get("statusCode"))
        .and_then(|v| v.as_u64())
        .and_then(|s| u16::try_from(s).ok())
        .unwrap_or(200);
    let headers = resp
        .get("headers")
        .and_then(|v| v.as_object())
        .map(|h| {
            h.iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
                .collect()
        })
        .unwrap_or_default();
    let body = resp
        .get("body")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    MockResponse {
        status,
        headers,
        body,
    }
}
