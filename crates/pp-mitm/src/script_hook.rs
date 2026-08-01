//! 脚本钩子引擎。
//!
//! 将 http-request / http-response 类型的脚本按 URL 规则挂载到 MITM 流量路径：
//! 请求/响应经过时命中规则的脚本依次执行，`$done` 返回的 headers / body / status
//! 回写到原始流量；脚本超时或抛异常时 no-op（透传原值）并记录警告。

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

/// 脚本钩子引擎：持有脚本执行 worker（收敛 `!Send` 的 QuickJS 执行）与规则列表。
pub struct ScriptHookEngine {
    worker: ScriptWorker,
    dialect: pp_script::ScriptDialect,
    rules: Vec<ScriptRule>,
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
    /// 脚本 arg 为 `{url, method, headers, body}`（body 仅在 `requires_body`
    /// 且不超过 `max_size` 时注入）。`$done` 返回对象含 `headers` / `body` 时回写；
    /// 超时 / 异常 no-op 并记录警告。
    pub async fn run_request_hooks(
        &self,
        url: &str,
        method: &str,
        headers: &mut Vec<(String, String)>,
        body: &mut Option<String>,
    ) {
        for rule in self
            .rules
            .iter()
            .filter(|r| r.kind == ScriptKind::HttpRequest)
        {
            if !rule.pattern.is_match(url) {
                continue;
            }
            let mut arg = serde_json::json!({
                "url": url,
                "method": method,
                "headers": headers_to_object(headers),
            });
            inject_body(&mut arg, rule, body);
            self.run_one(
                rule,
                ScriptKind::HttpRequest,
                Some(arg),
                None,
                headers,
                body,
            )
            .await;
        }
    }

    /// 运行响应阶段钩子：命中规则且类型为 [`ScriptKind::HttpResponse`] 的脚本依次执行。
    ///
    /// 脚本 arg 为 `{status, headers, body}`（body 规则同上）。`$done` 返回对象含
    /// `status` / `headers` / `body` 时回写；超时 / 异常 no-op 并记录警告。
    pub async fn run_response_hooks(
        &self,
        url: &str,
        status: &mut u16,
        headers: &mut Vec<(String, String)>,
        body: &mut Option<String>,
    ) {
        for rule in self
            .rules
            .iter()
            .filter(|r| r.kind == ScriptKind::HttpResponse)
        {
            if !rule.pattern.is_match(url) {
                continue;
            }
            let mut arg = serde_json::json!({
                "status": *status,
                "headers": headers_to_object(headers),
            });
            inject_body(&mut arg, rule, body);
            self.run_one(
                rule,
                ScriptKind::HttpResponse,
                Some(arg),
                Some(status),
                headers,
                body,
            )
            .await;
        }
    }

    /// 执行单条规则：经 [`ScriptWorker`] 串行执行，成功时回写输出，失败/超时仅记录警告。
    async fn run_one(
        &self,
        rule: &ScriptRule,
        kind: ScriptKind,
        arg: Option<serde_json::Value>,
        status: Option<&mut u16>,
        headers: &mut Vec<(String, String)>,
        body: &mut Option<String>,
    ) {
        match self
            .worker
            .run_script(
                &rule.source,
                kind,
                arg,
                rule.argument.as_deref(),
                self.dialect,
                &rule.name,
            )
            .await
        {
            Ok(ScriptOutput(out)) => apply_output(&out, status, headers, body),
            Err(e) => {
                tracing::warn!(script = %rule.name, "hook script failed: {e}");
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

/// 把 `$done` 返回对象中的 status / headers / body 回写。
fn apply_output(
    out: &serde_json::Value,
    status: Option<&mut u16>,
    headers: &mut Vec<(String, String)>,
    body: &mut Option<String>,
) {
    let Some(obj) = out.as_object() else {
        return;
    };
    if let Some(status) = status {
        if let Some(s) = obj
            .get("status")
            .and_then(|v| v.as_u64())
            .and_then(|s| u16::try_from(s).ok())
        {
            *status = s;
        }
    }
    if let Some(h) = obj.get("headers").and_then(|v| v.as_object()) {
        headers.clear();
        headers.extend(
            h.iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string())),
        );
    }
    if let Some(b) = obj.get("body").and_then(|v| v.as_str()) {
        *body = Some(b.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pp_script::{MemoryPersistentStore, MockHttpExecutor, RecordingNotifier};
    use regex::Regex;
    use std::time::Instant;

    fn test_host() -> Arc<ScriptHost> {
        let http = Arc::new(MockHttpExecutor::with_responses(vec![]));
        let store = Arc::new(MemoryPersistentStore::new());
        let notifier = Arc::new(RecordingNotifier::new());
        Arc::new(ScriptHost::new(http, store, notifier))
    }

    #[tokio::test(flavor = "current_thread")]
    async fn surge_http_response_script_rewrites_json_body() {
        let engine = ScriptHookEngine::new(
            test_host(),
            pp_script::ScriptDialect::Surge,
            pp_script::ScriptLimits::default(),
            vec![ScriptRule {
                name: "rewrite-json".to_string(),
                kind: ScriptKind::HttpResponse,
                pattern: Regex::new(r"^https://api\.example\.com/").unwrap(),
                requires_body: true,
                max_size: 65536,
                source: r#"
                    const data = JSON.parse($response.body);
                    data.a = 99;
                    $done({body: JSON.stringify(data)});
                "#
                .to_string(),
                argument: None,
            }],
        );

        let mut status = 200u16;
        let mut headers = vec![("content-type".to_string(), "application/json".to_string())];
        let mut body = Some(r#"{"a":1,"b":2}"#.to_string());

        engine
            .run_response_hooks(
                "https://api.example.com/v1/data",
                &mut status,
                &mut headers,
                &mut body,
            )
            .await;

        let parsed: serde_json::Value = serde_json::from_str(body.as_deref().unwrap()).unwrap();
        assert_eq!(parsed["a"], 99);
        assert_eq!(parsed["b"], 2);
        assert_eq!(status, 200);
        // $done 未返回 headers 时原 headers 保留。
        assert_eq!(
            headers,
            vec![("content-type".to_string(), "application/json".to_string())]
        );
    }

    #[tokio::test(flavor = "current_thread")]
    async fn request_infinite_loop_times_out_and_passes_through() {
        let engine = ScriptHookEngine::new(
            test_host(),
            pp_script::ScriptDialect::Surge,
            pp_script::ScriptLimits {
                timeout_ms: 500,
                ..pp_script::ScriptLimits::default()
            },
            vec![ScriptRule {
                name: "infinite-loop".to_string(),
                kind: ScriptKind::HttpRequest,
                pattern: Regex::new(".*").unwrap(),
                requires_body: true,
                max_size: 65536,
                source: "while (true) {}".to_string(),
                argument: None,
            }],
        );

        let mut headers = vec![("x-test".to_string(), "1".to_string())];
        let mut body = Some("payload".to_string());

        let start = Instant::now();
        engine
            .run_request_hooks(
                "https://api.example.com/data",
                "GET",
                &mut headers,
                &mut body,
            )
            .await;
        let elapsed = start.elapsed();

        // 透传：超时脚本 no-op，原值保持不变。
        assert_eq!(headers, vec![("x-test".to_string(), "1".to_string())]);
        assert_eq!(body.as_deref(), Some("payload"));
        // 500ms 超时生效（留足调度容差）。
        assert!(
            elapsed.as_millis() < 1000,
            "timeout did not bound execution: {elapsed:?}"
        );
    }

    /// `$argument` 透传 e2e：ScriptRule.argument 被注入为全局 `$argument`，
    /// 脚本将其回写到 `$done` 的 body（apply_output 只映射 status/headers/body）。
    #[tokio::test(flavor = "current_thread")]
    async fn script_rule_argument_injected_as_global() {
        let engine = ScriptHookEngine::new(
            test_host(),
            pp_script::ScriptDialect::Surge,
            pp_script::ScriptLimits::default(),
            vec![ScriptRule {
                name: "arg-hook".to_string(),
                kind: ScriptKind::HttpResponse,
                pattern: Regex::new(r"^https://api\.example\.com/").unwrap(),
                requires_body: false,
                max_size: 65536,
                source: r#"
                    if (typeof $argument === "undefined") {
                        $done({body: "UNDEFINED"});
                    } else {
                        $done({body: $argument});
                    }
                "#
                .to_string(),
                argument: Some("api.example.com|abc".to_string()),
            }],
        );

        let mut status = 200u16;
        let mut headers = Vec::new();
        let mut body = None;
        engine
            .run_response_hooks(
                "https://api.example.com/v1/data",
                &mut status,
                &mut headers,
                &mut body,
            )
            .await;
        assert_eq!(status, 200);
        assert_eq!(
            body.as_deref(),
            Some("api.example.com|abc"),
            "$argument 应被注入并回写到 body"
        );
    }
}
