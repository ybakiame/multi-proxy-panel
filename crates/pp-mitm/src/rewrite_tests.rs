//! `rewrite` 模块单元测试。

use super::rewrite::*;
use regex::Regex;

fn engine(rules: Vec<RewriteRule>) -> RewriteEngine {
    RewriteEngine { rules }
}

fn rule(kind: RewriteKind, pattern: &str) -> RewriteRule {
    RewriteRule {
        kind,
        pattern: Regex::new(pattern).unwrap(),
    }
}

#[test]
fn url_rewrite_supports_capture_groups() {
    let e = engine(vec![rule(
        RewriteKind::UrlRewrite {
            target: "https://cdn.example.com/v1/$1".to_string(),
        },
        r"^http://static\.example\.com/(.*)$",
    )]);
    let mut url = "http://static.example.com/foo/bar".to_string();
    let mut headers = Vec::new();
    let mut body: Option<String> = None;
    let action = e.apply_request(&mut url, &mut headers, &mut body);
    assert_eq!(action, RewriteAction::Continue);
    assert_eq!(url, "https://cdn.example.com/v1/foo/bar");
}

#[test]
fn redirect_short_circuits_with_location() {
    let e = engine(vec![rule(
        RewriteKind::Redirect {
            status: 302,
            target: "https://m.example.com/$1".to_string(),
        },
        r"^https?://www\.example\.com/(.*)$",
    )]);
    let mut url = "http://www.example.com/a/b".to_string();
    let mut headers = Vec::new();
    let mut body: Option<String> = None;
    let action = e.apply_request(&mut url, &mut headers, &mut body);
    assert_eq!(
        action,
        RewriteAction::Redirect {
            status: 302,
            location: "https://m.example.com/a/b".to_string(),
        }
    );
    assert_eq!(url, "http://www.example.com/a/b", "URL 不应被改写");
}

#[test]
fn redirect_skipped_in_response_pass() {
    let e = engine(vec![rule(
        RewriteKind::Redirect {
            status: 302,
            target: "https://m.example.com/".to_string(),
        },
        ".*",
    )]);
    let mut status = 200u16;
    let mut headers = Vec::new();
    let mut body: Option<String> = None;
    let action = e.apply_response(
        "http://www.example.com/",
        &mut status,
        &mut headers,
        &mut body,
    );
    assert_eq!(action, RewriteAction::Continue);
}

#[test]
fn request_header_rewrite_sets_and_deletes_case_insensitively() {
    let e = engine(vec![
        rule(
            RewriteKind::HeaderRewrite {
                phase: Phase::Request,
                name: "X-Proxy".to_string(),
                value: Some("on".to_string()),
            },
            ".*",
        ),
        rule(
            RewriteKind::HeaderRewrite {
                phase: Phase::Request,
                name: "X-Remove".to_string(),
                value: None,
            },
            ".*",
        ),
    ]);
    let mut url = "http://example.com/".to_string();
    let mut headers = vec![
        ("x-proxy".to_string(), "off".to_string()),
        ("X-REMOVE".to_string(), "yes".to_string()),
        ("X-Keep".to_string(), "me".to_string()),
    ];
    let mut body: Option<String> = None;
    let action = e.apply_request(&mut url, &mut headers, &mut body);
    assert_eq!(action, RewriteAction::Continue);
    assert_eq!(
        headers,
        vec![
            ("X-Keep".to_string(), "me".to_string()),
            ("X-Proxy".to_string(), "on".to_string()),
        ]
    );
}

#[test]
fn header_value_rewrite_applies_regex_to_value() {
    let e = engine(vec![rule(
        RewriteKind::HeaderValueRewrite {
            phase: Phase::Request,
            name: "User-Agent".to_string(),
            value_pattern: Regex::new("Mozilla/\\S+").unwrap(),
            replacement: "CustomUA".to_string(),
        },
        ".*",
    )]);
    let mut url = "http://example.com/".to_string();
    let mut headers = vec![
        ("user-agent".to_string(), "Mozilla/5.0 Safari".to_string()),
        ("accept".to_string(), "*/*".to_string()),
    ];
    let mut body: Option<String> = None;
    e.apply_request(&mut url, &mut headers, &mut body);
    assert_eq!(
        headers,
        vec![
            ("user-agent".to_string(), "CustomUA Safari".to_string()),
            ("accept".to_string(), "*/*".to_string()),
        ]
    );
}

#[test]
fn header_block_rewrite_matches_across_headers() {
    let e = engine(vec![rule(
        RewriteKind::HeaderBlockRewrite {
            phase: Phase::Response,
            block_pattern: Regex::new("(?i)if-none-match: [^\r\n]+\r\n?").unwrap(),
            replacement: String::new(),
        },
        ".*",
    )]);
    let mut status = 200u16;
    let mut headers = vec![
        ("content-type".to_string(), "application/json".to_string()),
        ("If-None-Match".to_string(), "abc123".to_string()),
        ("x-other".to_string(), "1".to_string()),
    ];
    let mut body: Option<String> = None;
    e.apply_response("http://example.com/", &mut status, &mut headers, &mut body);
    assert_eq!(
        headers,
        vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("x-other".to_string(), "1".to_string()),
        ]
    );
}

#[test]
fn body_rewrite_with_separate_body_pattern() {
    let e = engine(vec![rule(
        RewriteKind::BodyRewrite {
            phase: Phase::Response,
            body_pattern: Some(Regex::new(r#""is_pro":\s*false"#).unwrap()),
            replacement: r#""is_pro": true"#.to_string(),
        },
        r"^https://api\.example\.com/vip",
    )]);
    let mut status = 200u16;
    let mut headers = Vec::new();
    let mut body = Some(r#"{"is_pro": false}"#.to_string());
    e.apply_response(
        "https://api.example.com/vip/user",
        &mut status,
        &mut headers,
        &mut body,
    );
    assert_eq!(body.as_deref(), Some(r#"{"is_pro": true}"#));

    // URL 不命中门控时 body 不变。
    let mut body = Some(r#"{"is_pro": false}"#.to_string());
    e.apply_response(
        "https://other.example.com/vip/user",
        &mut status,
        &mut headers,
        &mut body,
    );
    assert_eq!(body.as_deref(), Some(r#"{"is_pro": false}"#));
}

#[test]
fn body_rewrite_without_body_pattern_reuses_url_pattern() {
    let e = engine(vec![rule(
        RewriteKind::BodyRewrite {
            phase: Phase::Response,
            body_pattern: None,
            replacement: "REDACTED".to_string(),
        },
        "secret",
    )]);
    let mut status = 200u16;
    let mut headers = Vec::new();
    let mut body = Some("hello secret world".to_string());
    let action = e.apply_response(
        "http://example.com/page?secret=1",
        &mut status,
        &mut headers,
        &mut body,
    );
    assert_eq!(action, RewriteAction::Continue);
    assert_eq!(body.as_deref(), Some("hello REDACTED world"));

    // 请求 URL 不命中 pattern 时 body 保持不变。
    let mut body = Some("hello secret world".to_string());
    e.apply_response(
        "http://example.com/other",
        &mut status,
        &mut headers,
        &mut body,
    );
    assert_eq!(body.as_deref(), Some("hello secret world"));
}

#[test]
fn response_phase_rules_are_skipped_in_request_pass() {
    let e = engine(vec![rule(
        RewriteKind::HeaderRewrite {
            phase: Phase::Response,
            name: "X-Server".to_string(),
            value: Some("mitm".to_string()),
        },
        ".*",
    )]);
    let mut url = "http://example.com/".to_string();
    let mut headers: Vec<(String, String)> = Vec::new();
    let mut body: Option<String> = None;
    let action = e.apply_request(&mut url, &mut headers, &mut body);
    assert_eq!(action, RewriteAction::Continue);
    assert!(headers.is_empty());
}

#[test]
fn reject_variants_map_to_mock_action() {
    let cases: Vec<(RewriteKind, u16, &[u8], Option<&str>)> = vec![
        (RewriteKind::reject(), 404, b"", None),
        (RewriteKind::reject_200(), 200, b"", None),
        (RewriteKind::reject_img(), 200, b"GIF89a", Some("image/gif")),
        (
            RewriteKind::reject_json("{}"),
            200,
            b"{}",
            Some("application/json"),
        ),
        (
            RewriteKind::reject_json("[]"),
            200,
            b"[]",
            Some("application/json"),
        ),
    ];
    for (kind, status, body_prefix, content_type) in cases {
        let e = engine(vec![rule(kind, "blocked")]);
        let mut url = "http://example.com/blocked".to_string();
        let mut headers = Vec::new();
        let mut body: Option<String> = None;
        let action = e.apply_request(&mut url, &mut headers, &mut body);
        let RewriteAction::Mock {
            status: s,
            body: b,
            headers: h,
        } = action
        else {
            panic!("expected Mock action, got {action:?}");
        };
        assert_eq!(s, status);
        assert!(b.starts_with(body_prefix), "body prefix mismatch: {b:?}");
        assert_eq!(
            h.iter()
                .find(|(n, _)| n == "Content-Type")
                .map(|(_, v)| v.as_str()),
            content_type
        );

        // 未命中 pattern 时正常继续。
        let mut url = "http://example.com/ok".to_string();
        let mut headers = Vec::new();
        let mut body: Option<String> = None;
        assert_eq!(
            e.apply_request(&mut url, &mut headers, &mut body),
            RewriteAction::Continue
        );
    }
}

#[test]
fn mock_short_circuits_with_synthetic_response() {
    let e = engine(vec![rule(
        RewriteKind::Mock {
            status: 403,
            body: b"forbidden".to_vec(),
            headers: vec![("Content-Type".to_string(), "text/plain".to_string())],
        },
        ".*",
    )]);
    let mut url = "http://example.com/anything".to_string();
    let mut headers = Vec::new();
    let mut body: Option<String> = None;
    let action = e.apply_request(&mut url, &mut headers, &mut body);
    assert_eq!(
        action,
        RewriteAction::Mock {
            status: 403,
            body: b"forbidden".to_vec(),
            headers: vec![("Content-Type".to_string(), "text/plain".to_string())],
        }
    );
}

#[test]
fn mock_carries_headers_through_request_and_response() {
    let e = engine(vec![rule(
        RewriteKind::Mock {
            status: 200,
            body: b"{}".to_vec(),
            headers: vec![
                ("Content-Type".to_string(), "application/json".to_string()),
                ("X-Mock".to_string(), "yes".to_string()),
            ],
        },
        ".*",
    )]);
    let expected = RewriteAction::Mock {
        status: 200,
        body: b"{}".to_vec(),
        headers: vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("X-Mock".to_string(), "yes".to_string()),
        ],
    };

    let mut url = "http://example.com/".to_string();
    let mut headers = Vec::new();
    let mut body: Option<String> = None;
    let action = e.apply_request(&mut url, &mut headers, &mut body);
    assert_eq!(action, expected);

    let mut status = 200u16;
    let mut headers = Vec::new();
    let mut body: Option<String> = None;
    let action = e.apply_response("http://example.com/", &mut status, &mut headers, &mut body);
    assert_eq!(action, expected);
}
