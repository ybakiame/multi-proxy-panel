//! `script_hook` 模块测试（独立文件以满足业务文件 500 行门禁）。

use super::script_hook::*;
use pp_script::{
    MemoryPersistentStore, MockHttpExecutor, RecordingNotifier, ScriptHost, ScriptKind,
};
use regex::Regex;
use std::sync::Arc;
use std::time::Instant;

fn test_host() -> Arc<ScriptHost> {
    let http = Arc::new(MockHttpExecutor::with_responses(vec![]));
    let store = Arc::new(MemoryPersistentStore::new());
    let notifier = Arc::new(RecordingNotifier::new());
    Arc::new(ScriptHost::new(http, store, notifier))
}

fn rule(name: &str, kind: ScriptKind, source: &str) -> ScriptRule {
    ScriptRule {
        name: name.to_string(),
        kind,
        pattern: Regex::new(r"^https://api\.example\.com/").unwrap(),
        requires_body: true,
        max_size: 65536,
        source: source.to_string(),
        argument: None,
    }
}

fn engine(rules: Vec<ScriptRule>) -> ScriptHookEngine {
    ScriptHookEngine::new(
        test_host(),
        pp_script::ScriptDialect::Surge,
        pp_script::ScriptLimits::default(),
        rules,
    )
}

#[tokio::test(flavor = "current_thread")]
async fn surge_http_response_script_rewrites_json_body() {
    let engine = engine(vec![rule(
        "rewrite-json",
        ScriptKind::HttpResponse,
        r#"
            const data = JSON.parse($response.body);
            data.a = 99;
            $done({body: JSON.stringify(data)});
        "#,
    )]);

    let mut status = 200u16;
    let mut headers = vec![("content-type".to_string(), "application/json".to_string())];
    let mut body = Some(r#"{"a":1,"b":2}"#.to_string());

    let outcome = engine
        .run_response_hooks(
            "https://api.example.com/v1/data",
            &mut status,
            &mut headers,
            &mut body,
        )
        .await;

    assert_eq!(outcome, HookOutcome::Continue);
    let parsed: serde_json::Value = serde_json::from_str(body.as_deref().unwrap()).unwrap();
    assert_eq!(parsed["a"], 99);
    assert_eq!(parsed["b"], 2);
    assert_eq!(status, 200);
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
            pattern: Regex::new(".*").unwrap(),
            ..rule("infinite-loop", ScriptKind::HttpRequest, "while (true) {}")
        }],
    );

    let mut url = "https://api.example.com/data".to_string();
    let mut headers = vec![("x-test".to_string(), "1".to_string())];
    let mut body = Some("payload".to_string());

    let start = Instant::now();
    let outcome = engine
        .run_request_hooks(&mut url, "GET", &mut headers, &mut body)
        .await;
    let elapsed = start.elapsed();

    assert_eq!(outcome, HookOutcome::Continue);
    assert_eq!(headers, vec![("x-test".to_string(), "1".to_string())]);
    assert_eq!(body.as_deref(), Some("payload"));
    assert!(
        elapsed.as_millis() < 1000,
        "timeout did not bound execution: {elapsed:?}"
    );
}

/// `$argument` 透传 e2e：ScriptRule.argument 被注入为全局 `$argument`。
#[tokio::test(flavor = "current_thread")]
async fn script_rule_argument_injected_as_global() {
    let engine = engine(vec![ScriptRule {
        argument: Some("api.example.com|abc".to_string()),
        ..rule(
            "arg-hook",
            ScriptKind::HttpResponse,
            r#"
                if (typeof $argument === "undefined") {
                    $done({body: "UNDEFINED"});
                } else {
                    $done({body: $argument});
                }
            "#,
        )
    }]);

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
    assert_eq!(body.as_deref(), Some("api.example.com|abc"));
}

/// `$done({url})`：请求阶段替换 URL。
#[tokio::test(flavor = "current_thread")]
async fn done_url_replaces_request_url() {
    let engine = engine(vec![rule(
        "url-rewrite",
        ScriptKind::HttpRequest,
        r#"$done({url: "https://api.example.com/v2/data?token=abc"});"#,
    )]);

    let mut url = "https://api.example.com/v1/data".to_string();
    let mut headers = Vec::new();
    let mut body = None;
    let outcome = engine
        .run_request_hooks(&mut url, "GET", &mut headers, &mut body)
        .await;
    assert_eq!(outcome, HookOutcome::Continue);
    assert_eq!(url, "https://api.example.com/v2/data?token=abc");
}

/// `$done({response: {...}})`：请求阶段脚本合成响应（mock）。
#[tokio::test(flavor = "current_thread")]
async fn done_response_mock_short_circuits_request() {
    let engine = engine(vec![rule(
        "echo",
        ScriptKind::HttpRequest,
        r#"$done({response: {status: 200, headers: {"Content-Type": "application/json"}, body: "{\"ok\":true}"}});"#,
    )]);

    let mut url = "https://api.example.com/v1/data".to_string();
    let mut headers = Vec::new();
    let mut body = None;
    let outcome = engine
        .run_request_hooks(&mut url, "GET", &mut headers, &mut body)
        .await;
    let HookOutcome::Mock(mock) = outcome else {
        panic!("expected Mock, got {outcome:?}");
    };
    assert_eq!(mock.status, 200);
    assert_eq!(mock.body.as_deref(), Some(r#"{"ok":true}"#));
    assert_eq!(
        mock.headers,
        vec![("Content-Type".to_string(), "application/json".to_string())]
    );
}

/// `$done({abort: true})`：中止请求。
#[tokio::test(flavor = "current_thread")]
async fn done_abort_returns_abort_outcome() {
    let engine = engine(vec![rule(
        "abort",
        ScriptKind::HttpRequest,
        "$done({abort: true});",
    )]);
    let mut url = "https://api.example.com/v1/data".to_string();
    let mut headers = Vec::new();
    let mut body = None;
    let outcome = engine
        .run_request_hooks(&mut url, "GET", &mut headers, &mut body)
        .await;
    assert_eq!(outcome, HookOutcome::Abort);
}

/// QX 风格 `$done("...")`：字符串直接作为新 body。
#[tokio::test(flavor = "current_thread")]
async fn done_plain_string_replaces_body() {
    let engine = ScriptHookEngine::new(
        test_host(),
        pp_script::ScriptDialect::QuantumultX,
        pp_script::ScriptLimits::default(),
        vec![rule(
            "string-body",
            ScriptKind::HttpResponse,
            r#"$done("<html>replaced</html>");"#,
        )],
    );
    let mut status = 200u16;
    let mut headers = Vec::new();
    let mut body = Some("orig".to_string());
    engine
        .run_response_hooks(
            "https://api.example.com/v1/data",
            &mut status,
            &mut headers,
            &mut body,
        )
        .await;
    assert_eq!(body.as_deref(), Some("<html>replaced</html>"));
}

/// QX 别名：响应阶段 `$response.statusCode` 可读、`$done({statusCode})` 可写。
#[tokio::test(flavor = "current_thread")]
async fn qx_status_code_alias_read_write() {
    let engine = ScriptHookEngine::new(
        test_host(),
        pp_script::ScriptDialect::QuantumultX,
        pp_script::ScriptLimits::default(),
        vec![rule(
            "status-code",
            ScriptKind::HttpResponse,
            "if ($response.statusCode === 200) { $done({statusCode: 201}); } else { $done({}); }",
        )],
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
    assert_eq!(status, 201);
}

/// QX `$request.path`：从 URL 提取 path（含 query）。
#[tokio::test(flavor = "current_thread")]
async fn qx_request_path_injected() {
    let engine = ScriptHookEngine::new(
        test_host(),
        pp_script::ScriptDialect::QuantumultX,
        pp_script::ScriptLimits::default(),
        vec![rule(
            "path",
            ScriptKind::HttpRequest,
            "$done({body: $request.path});",
        )],
    );
    let mut url = "https://api.example.com/v1/data?x=1".to_string();
    let mut headers = Vec::new();
    let mut body = None;
    engine
        .run_request_hooks(&mut url, "GET", &mut headers, &mut body)
        .await;
    assert_eq!(body.as_deref(), Some("/v1/data?x=1"));
}
