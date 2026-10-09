//! URL / Header / Body 重写引擎。

use regex::Regex;

/// 1×1 透明 GIF（QX `reject-img` 语义）。
const REJECT_IMG_BODY: &[u8] = b"GIF89a\x01\x00\x01\x00\x80\x00\x00\xff\xff\xff\xff\xff\xff\
\x21\xf9\x04\x01\x0a\x00\x01\x00\x2c\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02\x4c\x01\x00\x3b";

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
    /// 返回 302/307 重定向（`target` 支持捕获组引用，写入 `Location` 头）。
    Redirect { status: u16, target: String },
    /// 改写（`value: Some`）或删除（`value: None`）指定请求头（头名大小写不敏感）。
    HeaderRewrite {
        phase: Phase,
        name: String,
        value: Option<String>,
    },
    /// 对指定头的**值**做正则替换（Surge `header-replace-regex`）。
    HeaderValueRewrite {
        phase: Phase,
        name: String,
        value_pattern: Regex,
        replacement: String,
    },
    /// 对序列化头块（`Name: value\r\n...`）整体做正则替换（QX
    /// `request-header` / `response-header` 双正则语义）。
    HeaderBlockRewrite {
        phase: Phase,
        block_pattern: Regex,
        replacement: String,
    },
    /// 在 body 中做正则替换：URL 由规则 `pattern` 门控，body 正则取
    /// `body_pattern`，为 `None` 时复用 URL pattern（兼容旧行为）。
    BodyRewrite {
        phase: Phase,
        body_pattern: Option<Regex>,
        replacement: String,
    },
    /// 直接拒绝该请求：状态码 + 可选 body / Content-Type（覆盖 QX/Loon 的
    /// `reject` / `reject-200` / `reject-img` / `reject-dict` / `reject-array`）。
    Reject {
        status: u16,
        body: Vec<u8>,
        content_type: Option<String>,
    },
    /// 直接返回合成响应（Surge `[Map Local]` / QX `echo-response`）。
    Mock {
        status: u16,
        body: Vec<u8>,
        headers: Vec<(String, String)>,
    },
}

impl RewriteKind {
    /// QX/Loon `reject`：404 + 空 body。
    pub fn reject() -> Self {
        Self::Reject {
            status: 404,
            body: Vec::new(),
            content_type: None,
        }
    }

    /// QX/Loon `reject-200`：200 + 空 body。
    pub fn reject_200() -> Self {
        Self::Reject {
            status: 200,
            body: Vec::new(),
            content_type: None,
        }
    }

    /// QX `reject-img`：200 + 1×1 GIF。
    pub fn reject_img() -> Self {
        Self::Reject {
            status: 200,
            body: REJECT_IMG_BODY.to_vec(),
            content_type: Some("image/gif".to_string()),
        }
    }

    /// QX/Loon `reject-dict`（`{}`）/ `reject-array`（`[]`）。
    pub fn reject_json(body: &'static str) -> Self {
        Self::Reject {
            status: 200,
            body: body.as_bytes().to_vec(),
            content_type: Some("application/json".to_string()),
        }
    }
}

/// 单条重写规则：URL 正则模式 + 动作。
#[derive(Debug, Clone)]
pub struct RewriteRule {
    pub kind: RewriteKind,
    pub pattern: Regex,
}

/// 重写引擎执行结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RewriteAction {
    Continue,
    /// 终止本次请求并以合成响应作答（`Reject` 各变体与 `Mock` 统一走此动作）。
    Mock {
        status: u16,
        body: Vec<u8>,
        headers: Vec<(String, String)>,
    },
    /// 终止本次请求并返回重定向响应。
    Redirect {
        status: u16,
        location: String,
    },
}

/// 按顺序应用一组重写规则。
#[derive(Debug, Clone, Default)]
pub struct RewriteEngine {
    pub rules: Vec<RewriteRule>,
}

impl RewriteEngine {
    /// 应用请求阶段规则：URL 重写、phase=Request 的 Header/Body 规则；
    /// `Reject` / `Mock` / `Redirect` 命中即短路返回。
    pub fn apply_request(
        &self,
        url: &mut String,
        headers: &mut Vec<(String, String)>,
        body: &mut Option<String>,
    ) -> RewriteAction {
        for rule in &self.rules {
            if let Some(action) = self.apply_one(rule, Phase::Request, url, headers, body) {
                return action;
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
        let mut url = url.to_string();
        for rule in &self.rules {
            if let Some(action) = self.apply_one(rule, Phase::Response, &mut url, headers, body) {
                return action;
            }
        }
        RewriteAction::Continue
    }

    /// 应用单条规则；命中短路类动作时返回 `Some(action)`。
    fn apply_one(
        &self,
        rule: &RewriteRule,
        phase: Phase,
        url: &mut String,
        headers: &mut Vec<(String, String)>,
        body: &mut Option<String>,
    ) -> Option<RewriteAction> {
        match &rule.kind {
            RewriteKind::UrlRewrite { target } if phase == Phase::Request => {
                *url = rule.pattern.replace(url, target.as_str()).into_owned();
            }
            RewriteKind::Redirect { status, target }
                if phase == Phase::Request && rule.pattern.is_match(url) =>
            {
                return Some(RewriteAction::Redirect {
                    status: *status,
                    location: rule.pattern.replace(url, target.as_str()).into_owned(),
                });
            }
            RewriteKind::HeaderRewrite {
                phase: p,
                name,
                value,
            } if *p == phase && rule.pattern.is_match(url) => {
                apply_header(headers, name, value);
            }
            RewriteKind::HeaderValueRewrite {
                phase: p,
                name,
                value_pattern,
                replacement,
            } if *p == phase && rule.pattern.is_match(url) => {
                for (n, v) in headers.iter_mut() {
                    if n.eq_ignore_ascii_case(name) && value_pattern.is_match(v) {
                        *v = value_pattern.replace(v, replacement.as_str()).into_owned();
                    }
                }
            }
            RewriteKind::HeaderBlockRewrite {
                phase: p,
                block_pattern,
                replacement,
            } if *p == phase && rule.pattern.is_match(url) => {
                let block = headers_to_block(headers);
                if block_pattern.is_match(&block) {
                    *headers =
                        block_to_headers(&block_pattern.replace(&block, replacement.as_str()));
                }
            }
            RewriteKind::BodyRewrite {
                phase: p,
                body_pattern,
                replacement,
            } if *p == phase && rule.pattern.is_match(url) => {
                if let Some(b) = body {
                    let pat = body_pattern.as_ref().unwrap_or(&rule.pattern);
                    *b = pat.replace(b, replacement.as_str()).into_owned();
                }
            }
            RewriteKind::Reject {
                status,
                body: reject_body,
                content_type,
            } if rule.pattern.is_match(url) => {
                let headers = content_type
                    .as_ref()
                    .map(|ct| vec![("Content-Type".to_string(), ct.clone())])
                    .unwrap_or_default();
                return Some(RewriteAction::Mock {
                    status: *status,
                    body: reject_body.clone(),
                    headers,
                });
            }
            RewriteKind::Mock {
                status,
                body: mock_body,
                headers,
            } if rule.pattern.is_match(url) => {
                return Some(RewriteAction::Mock {
                    status: *status,
                    body: mock_body.clone(),
                    headers: headers.clone(),
                });
            }
            _ => {}
        }
        None
    }
}

/// 改写指定请求头：先移除同名项（大小写不敏感），`value` 存在时追加新值，
/// 否则视为删除。
pub(crate) fn apply_header(
    headers: &mut Vec<(String, String)>,
    name: &str,
    value: &Option<String>,
) {
    headers.retain(|(n, _)| !n.eq_ignore_ascii_case(name));
    if let Some(value) = value {
        headers.push((name.to_string(), value.clone()));
    }
}

/// 头列表 → `Name: value\r\n...` 序列化块（QX header rewrite 的操作对象）。
fn headers_to_block(headers: &[(String, String)]) -> String {
    headers
        .iter()
        .map(|(n, v)| format!("{n}: {v}"))
        .collect::<Vec<_>>()
        .join("\r\n")
}

/// 序列化头块 → 头列表；无法解析的行丢弃。
fn block_to_headers(block: &str) -> Vec<(String, String)> {
    block
        .split("\r\n")
        .filter_map(|line| {
            let (name, value) = line.split_once(':')?;
            let (name, value) = (name.trim(), value.trim());
            (!name.is_empty()).then(|| (name.to_string(), value.to_string()))
        })
        .collect()
}
