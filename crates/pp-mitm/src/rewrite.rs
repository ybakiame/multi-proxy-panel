//! URL / Header / Body 重写引擎。

use regex::Regex;

/// 规则作用的代理阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Request,
    Response,
}

/// 一条重写规则的动作类型。
#[derive(Debug, Clone)]
pub enum RewriteKind {
    /// 重写请求 URL，支持 `$1` 等捕获组引用。
    UrlRewrite { target: String },
    /// 改写（`value: Some`）或删除（`value: None`）指定请求头。
    HeaderRewrite {
        phase: Phase,
        name: String,
        value: Option<String>,
    },
    /// 在 body 中做正则替换。
    BodyRewrite { phase: Phase, replacement: String },
    /// 直接拒绝该请求。
    Reject,
    /// 直接返回合成响应。
    Mock { status: u16, body: String },
}

/// 单条重写规则：正则模式 + 动作。
#[derive(Debug, Clone)]
pub struct RewriteRule {
    pub kind: RewriteKind,
    pub pattern: Regex,
}

/// 重写引擎执行结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RewriteAction {
    Continue,
    Reject,
    Mock { status: u16, body: String },
}

/// 按顺序应用一组重写规则。
#[derive(Debug, Clone, Default)]
pub struct RewriteEngine {
    pub rules: Vec<RewriteRule>,
}

impl RewriteEngine {
    /// 应用请求阶段规则：URL 重写、phase=Request 的 Header/Body 规则；
    /// `Reject` / `Mock` 命中即短路返回。
    pub fn apply_request(
        &self,
        url: &mut String,
        headers: &mut Vec<(String, String)>,
        body: &mut Option<String>,
    ) -> RewriteAction {
        for rule in &self.rules {
            match &rule.kind {
                RewriteKind::UrlRewrite { target } => {
                    *url = rule.pattern.replace(url, target.as_str()).into_owned();
                }
                RewriteKind::HeaderRewrite {
                    phase: Phase::Request,
                    name,
                    value,
                } if rule.pattern.is_match(url) => {
                    apply_header(headers, name, value);
                }
                RewriteKind::BodyRewrite {
                    phase: Phase::Request,
                    replacement,
                } if rule.pattern.is_match(url) => {
                    if let Some(b) = body {
                        *b = rule.pattern.replace(b, replacement.as_str()).into_owned();
                    }
                }
                RewriteKind::Reject if rule.pattern.is_match(url) => {
                    return RewriteAction::Reject;
                }
                RewriteKind::Mock {
                    status,
                    body: mock_body,
                } if rule.pattern.is_match(url) => {
                    return RewriteAction::Mock {
                        status: *status,
                        body: mock_body.clone(),
                    };
                }
                // 响应阶段规则留待 apply_response 处理。
                _ => {}
            }
        }
        RewriteAction::Continue
    }

    /// 应用响应阶段规则，规则按请求 URL 匹配。
    pub fn apply_response(
        &self,
        url: &str,
        _status: &mut u16,
        headers: &mut Vec<(String, String)>,
        body: &mut Option<String>,
    ) -> RewriteAction {
        for rule in &self.rules {
            match &rule.kind {
                RewriteKind::HeaderRewrite {
                    phase: Phase::Response,
                    name,
                    value,
                } if rule.pattern.is_match(url) => {
                    apply_header(headers, name, value);
                }
                RewriteKind::BodyRewrite {
                    phase: Phase::Response,
                    replacement,
                } if rule.pattern.is_match(url) => {
                    if let Some(b) = body {
                        *b = rule.pattern.replace(b, replacement.as_str()).into_owned();
                    }
                }
                RewriteKind::Reject if rule.pattern.is_match(url) => {
                    return RewriteAction::Reject;
                }
                RewriteKind::Mock {
                    status,
                    body: mock_body,
                } if rule.pattern.is_match(url) => {
                    return RewriteAction::Mock {
                        status: *status,
                        body: mock_body.clone(),
                    };
                }
                // 请求阶段规则与 URL 重写在 apply_request 中处理。
                _ => {}
            }
        }
        RewriteAction::Continue
    }
}

/// 改写指定请求头：先移除同名项，`value` 存在时追加新值，否则视为删除。
fn apply_header(headers: &mut Vec<(String, String)>, name: &str, value: &Option<String>) {
    headers.retain(|(n, _)| n.as_str() != name);
    if let Some(value) = value {
        headers.push((name.to_string(), value.clone()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine(rules: Vec<RewriteRule>) -> RewriteEngine {
        RewriteEngine { rules }
    }

    #[test]
    fn url_rewrite_supports_capture_groups() {
        let e = engine(vec![RewriteRule {
            kind: RewriteKind::UrlRewrite {
                target: "https://cdn.example.com/v1/$1".to_string(),
            },
            pattern: Regex::new(r"^http://static\.example\.com/(.*)$").unwrap(),
        }]);
        let mut url = "http://static.example.com/foo/bar".to_string();
        let mut headers = Vec::new();
        let mut body: Option<String> = None;
        let action = e.apply_request(&mut url, &mut headers, &mut body);
        assert_eq!(action, RewriteAction::Continue);
        assert_eq!(url, "https://cdn.example.com/v1/foo/bar");
    }

    #[test]
    fn request_header_rewrite_sets_and_deletes() {
        let e = engine(vec![
            RewriteRule {
                kind: RewriteKind::HeaderRewrite {
                    phase: Phase::Request,
                    name: "X-Proxy".to_string(),
                    value: Some("on".to_string()),
                },
                pattern: Regex::new(".*").unwrap(),
            },
            RewriteRule {
                kind: RewriteKind::HeaderRewrite {
                    phase: Phase::Request,
                    name: "X-Remove".to_string(),
                    value: None,
                },
                pattern: Regex::new(".*").unwrap(),
            },
        ]);
        let mut url = "http://example.com/".to_string();
        let mut headers = vec![
            ("X-Proxy".to_string(), "off".to_string()),
            ("X-Remove".to_string(), "yes".to_string()),
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
    fn response_body_rewrite_matches_request_url() {
        let e = engine(vec![RewriteRule {
            kind: RewriteKind::BodyRewrite {
                phase: Phase::Response,
                replacement: "REDACTED".to_string(),
            },
            pattern: Regex::new("secret").unwrap(),
        }]);
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
        let mut status = 200u16;
        let mut headers = Vec::new();
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
        let e = engine(vec![RewriteRule {
            kind: RewriteKind::HeaderRewrite {
                phase: Phase::Response,
                name: "X-Server".to_string(),
                value: Some("mitm".to_string()),
            },
            pattern: Regex::new(".*").unwrap(),
        }]);
        let mut url = "http://example.com/".to_string();
        let mut headers: Vec<(String, String)> = Vec::new();
        let mut body: Option<String> = None;
        let action = e.apply_request(&mut url, &mut headers, &mut body);
        assert_eq!(action, RewriteAction::Continue);
        assert!(headers.is_empty());
    }

    #[test]
    fn reject_short_circuits() {
        let e = engine(vec![RewriteRule {
            kind: RewriteKind::Reject,
            pattern: Regex::new("blocked").unwrap(),
        }]);
        let mut url = "http://example.com/blocked/path".to_string();
        let mut headers = Vec::new();
        let mut body: Option<String> = None;
        let action = e.apply_request(&mut url, &mut headers, &mut body);
        assert_eq!(action, RewriteAction::Reject);

        // 未命中 pattern 时正常继续。
        let mut url = "http://example.com/ok/path".to_string();
        let mut headers = Vec::new();
        let mut body: Option<String> = None;
        let action = e.apply_request(&mut url, &mut headers, &mut body);
        assert_eq!(action, RewriteAction::Continue);
    }

    #[test]
    fn mock_short_circuits_with_synthetic_response() {
        let e = engine(vec![RewriteRule {
            kind: RewriteKind::Mock {
                status: 403,
                body: "forbidden".to_string(),
            },
            pattern: Regex::new(".*").unwrap(),
        }]);
        let mut url = "http://example.com/anything".to_string();
        let mut headers = Vec::new();
        let mut body: Option<String> = None;
        let action = e.apply_request(&mut url, &mut headers, &mut body);
        assert_eq!(
            action,
            RewriteAction::Mock {
                status: 403,
                body: "forbidden".to_string(),
            }
        );
    }
}
