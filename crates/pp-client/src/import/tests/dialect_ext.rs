//! Phase B 语法补全测试：QX 全量 action / Loon `[Rewrite]` / Surge `[Body Rewrite]` /
//! 混合方言行 / 顶层裸 hostname / Loon 逗号粘连参数与 URL 编码 argument。

use super::*;
use pp_mitm::RewriteKind;

#[test]
fn qx_reject_variants_map_to_typed_rejects() {
    let content = "\
[rewrite_local]
^https://a\\.com/ads url reject
^https://a\\.com/api url reject-200
^https://a\\.com/img url reject-img
^https://a\\.com/dict url reject-dict
^https://a\\.com/arr url reject-array
";
    let cfg = parse_import(content, ScriptDialect::QuantumultX).unwrap();
    assert_eq!(cfg.rewrites.len(), 5);
    let kinds: Vec<&RewriteKind> = cfg.rewrites.iter().map(|r| &r.kind).collect();
    match kinds[0] {
        RewriteKind::Reject { status, body, .. } => {
            assert_eq!(*status, 404);
            assert!(body.is_empty());
        }
        other => panic!("expected Reject, got {other:?}"),
    }
    match kinds[1] {
        RewriteKind::Reject { status, body, .. } => {
            assert_eq!(*status, 200);
            assert!(body.is_empty());
        }
        other => panic!("expected reject-200, got {other:?}"),
    }
    match kinds[2] {
        RewriteKind::Reject {
            status,
            body,
            content_type,
        } => {
            assert_eq!(*status, 200);
            assert_eq!(content_type.as_deref(), Some("image/gif"));
            assert!(!body.is_empty());
        }
        other => panic!("expected reject-img, got {other:?}"),
    }
    match kinds[3] {
        RewriteKind::Reject { body, .. } => assert_eq!(body, b"{}"),
        other => panic!("expected reject-dict, got {other:?}"),
    }
    match kinds[4] {
        RewriteKind::Reject { body, .. } => assert_eq!(body, b"[]"),
        other => panic!("expected reject-array, got {other:?}"),
    }
}

#[test]
fn qx_redirect_and_echo_response() {
    let content = "\
[rewrite_local]
^https://old\\.com/(.*) url 302 https://new.com/$1
^https://api\\.com/data url echo-response application/json echo-response {\"code\":0}
";
    let cfg = parse_import(content, ScriptDialect::QuantumultX).unwrap();
    assert_eq!(cfg.rewrites.len(), 2);
    match &cfg.rewrites[0].kind {
        RewriteKind::Redirect { status, target } => {
            assert_eq!(*status, 302);
            assert_eq!(target, "https://new.com/$1");
        }
        other => panic!("expected Redirect, got {other:?}"),
    }
    match &cfg.rewrites[1].kind {
        RewriteKind::Mock {
            status,
            body,
            headers,
        } => {
            assert_eq!(*status, 200);
            assert_eq!(body, b"{\"code\":0}");
            assert_eq!(
                headers,
                &vec![("Content-Type".to_string(), "application/json".to_string())]
            );
        }
        other => panic!("expected Mock, got {other:?}"),
    }
}

#[test]
fn qx_double_regex_response_body_preserves_body_pattern() {
    let content = r#"
[rewrite_local]
^https://api\.com/user url response-body "is_pro_user":.*?, response-body "is_pro_user": true,
"#;
    let cfg = parse_import(content, ScriptDialect::QuantumultX).unwrap();
    assert_eq!(cfg.rewrites.len(), 1);
    match &cfg.rewrites[0].kind {
        RewriteKind::BodyRewrite {
            phase,
            body_pattern,
            replacement,
        } => {
            assert_eq!(*phase, Phase::Response);
            let bp = body_pattern.as_ref().expect("body pattern must be kept");
            assert_eq!(bp.as_str(), r#""is_pro_user":.*?,"#);
            assert_eq!(replacement, r#""is_pro_user": true,"#);
        }
        other => panic!("expected BodyRewrite, got {other:?}"),
    }
}

#[test]
fn qx_header_double_regex_maps_to_header_block_rewrite() {
    let content = "\
[rewrite_local]
^https://api\\.com/ url request-header Accept-Encoding:.+ request-header Accept-Encoding: identity
";
    let cfg = parse_import(content, ScriptDialect::QuantumultX).unwrap();
    match &cfg.rewrites[0].kind {
        RewriteKind::HeaderBlockRewrite {
            phase,
            block_pattern,
            replacement,
        } => {
            assert_eq!(*phase, Phase::Request);
            assert_eq!(block_pattern.as_str(), "Accept-Encoding:.+");
            assert_eq!(replacement, "Accept-Encoding: identity");
        }
        other => panic!("expected HeaderBlockRewrite, got {other:?}"),
    }
}

#[test]
fn qx_script_family_maps_kinds_and_requires_body() {
    let content = "\
[rewrite_local]
^https://a\\.com/1 url script-request-header https://x.com/a.js
^https://a\\.com/2 url script-request-body https://x.com/b.js
^https://a\\.com/3 url script-response-header https://x.com/c.js
^https://a\\.com/4 url script-response-body https://x.com/d.js
^https://a\\.com/5 url script-echo-response https://x.com/e.js
^https://a\\.com/6 url script-analyze-echo-response https://x.com/f.js
";
    let cfg = parse_import(content, ScriptDialect::QuantumultX).unwrap();
    let got: Vec<(ScriptKind, bool)> = cfg
        .scripts
        .iter()
        .map(|s| (s.kind, s.requires_body))
        .collect();
    assert_eq!(
        got,
        vec![
            (ScriptKind::HttpRequest, false),
            (ScriptKind::HttpRequest, true),
            (ScriptKind::HttpResponse, false),
            (ScriptKind::HttpResponse, true),
            (ScriptKind::HttpRequest, false),
            (ScriptKind::HttpRequest, true),
        ]
    );
    assert_eq!(cfg.script_urls.len(), 6);
}

#[test]
fn qx_jsonjq_skipped_with_warning() {
    let content = "\
[rewrite_local]
^https://a\\.com/ url jsonjq-response-body https://x.com/jq.js
";
    let cfg = parse_import(content, ScriptDialect::QuantumultX).unwrap();
    assert!(cfg.rewrites.is_empty());
    assert!(cfg.scripts.is_empty());
    assert_eq!(cfg.warnings.len(), 1);
    assert!(cfg.warnings[0].contains("jsonjq"));
}

#[test]
fn qx_mixed_dialect_lines_dispatched_by_line_shape() {
    // 网易云段形态：QX 文件内混入 Surge 风格脚本行与 Loon 风格脚本行。
    let content = "\
[rewrite_local]
网易云音质 = type=http-request, pattern=^https://music\\.163\\.com, script-path=https://x.com/ncm.js
http-response ^https://api\\.x\\.com/me script-path=https://x.com/me.js, requires-body=true, tag=LoonHook
^https://a\\.com/ads url reject
";
    let cfg = parse_import(content, ScriptDialect::QuantumultX).unwrap();
    assert_eq!(cfg.scripts.len(), 2);
    assert_eq!(cfg.scripts[0].name, "网易云音质");
    assert_eq!(cfg.scripts[0].kind, ScriptKind::HttpRequest);
    assert_eq!(cfg.scripts[1].name, "LoonHook");
    assert_eq!(cfg.scripts[1].kind, ScriptKind::HttpResponse);
    assert!(cfg.scripts[1].requires_body);
    assert_eq!(cfg.rewrites.len(), 1);
    assert!(matches!(cfg.rewrites[0].kind, RewriteKind::Reject { .. }));
}

#[test]
fn qx_top_level_hostname_line_collected_as_mitm_whitelist() {
    // QX snippet 的 hostname 常写在文件顶层（不在 [mitm] section 内）。
    let content = "\
hostname = api.example.com, *.cdn.example.com, -exclude.example.com
[rewrite_local]
^https://a\\.com/ url reject
";
    let cfg = parse_import(content, ScriptDialect::QuantumultX).unwrap();
    assert_eq!(
        cfg.hostnames,
        vec![
            "api.example.com",
            "*.cdn.example.com",
            "-exclude.example.com"
        ]
    );
    assert_eq!(cfg.rewrites.len(), 1);
}

#[test]
fn mitm_non_hostname_keys_ignored() {
    let content = "\
[mitm]
skip_validating_cert = false
hostname = a.com
";
    let cfg = parse_import(content, ScriptDialect::Surge).unwrap();
    assert_eq!(cfg.hostnames, vec!["a.com"]);
}

#[test]
fn loon_rewrite_section_full_actions() {
    let content = "\
[Rewrite]
^https://a\\.com/ads reject
^https://a\\.com/p reject-200
^https://a\\.com/i reject-img
^https://a\\.com/d reject-dict
^https://a\\.com/ar reject-array
^https://a\\.com/old 302 https://new.com/
^https://a\\.com/h header-del if-none-match
^https://a\\.com/h2 header-replace X-Token abc
^https://a\\.com/h3 header-replace-regex X-Env prod.* stage
";
    let cfg = parse_import(content, ScriptDialect::Loon).unwrap();
    assert_eq!(cfg.rewrites.len(), 9);
    assert!(matches!(cfg.rewrites[0].kind, RewriteKind::Reject { .. }));
    match &cfg.rewrites[5].kind {
        RewriteKind::Redirect { status, target } => {
            assert_eq!(*status, 302);
            assert_eq!(target, "https://new.com/");
        }
        other => panic!("expected Redirect, got {other:?}"),
    }
    match &cfg.rewrites[6].kind {
        RewriteKind::HeaderRewrite { name, value, .. } => {
            assert_eq!(name, "if-none-match");
            assert_eq!(*value, None);
        }
        other => panic!("expected header-del, got {other:?}"),
    }
    match &cfg.rewrites[8].kind {
        RewriteKind::HeaderValueRewrite {
            name,
            value_pattern,
            replacement,
            ..
        } => {
            assert_eq!(name, "X-Env");
            assert_eq!(value_pattern.as_str(), "prod.*");
            assert_eq!(replacement, "stage");
        }
        other => panic!("expected header-replace-regex, got {other:?}"),
    }
}

#[test]
fn loon_script_comma_attached_params_and_url_encoded_argument() {
    let content = "\
[Script]
http-response ^https://api\\.x\\.com/me,requires-body=0 script-path=https://x.com/me.js, tag=Me
http-request ^https://api\\.y\\.com/pro script-path=https://x.com/pro.js, requires-body=1, max-size=-1, argument=\"%22is_pro_user%22%3Atrue\"
";
    let cfg = parse_import(content, ScriptDialect::Loon).unwrap();
    assert_eq!(cfg.scripts.len(), 2);
    assert_eq!(cfg.scripts[0].name, "Me");
    assert!(!cfg.scripts[0].requires_body);
    assert_eq!(cfg.scripts[0].pattern.as_str(), "^https://api\\.x\\.com/me");
    let pro = &cfg.scripts[1];
    assert!(pro.requires_body);
    assert_eq!(pro.max_size, 10 * 1024 * 1024);
    assert_eq!(pro.argument.as_deref(), Some("\"is_pro_user\":true"));
}

#[test]
fn surge_url_rewrite_trailing_mode_tokens() {
    let content = "\
[URL Rewrite]
^https://a\\.com/ads - reject
^https://old\\.com/(.*) https://new.com/$1 302
^https://legacy\\.com/(.*) https://keep.com/$1 307
^https://plain\\.com/(.*) https://target.com/$1
";
    let cfg = parse_import(content, ScriptDialect::Surge).unwrap();
    assert_eq!(cfg.rewrites.len(), 4);
    assert!(matches!(cfg.rewrites[0].kind, RewriteKind::Reject { .. }));
    match &cfg.rewrites[1].kind {
        RewriteKind::Redirect { status, target } => {
            assert_eq!(*status, 302);
            assert_eq!(target, "https://new.com/$1");
        }
        other => panic!("expected Redirect 302, got {other:?}"),
    }
    match &cfg.rewrites[2].kind {
        RewriteKind::Redirect { status, .. } => assert_eq!(*status, 307),
        other => panic!("expected Redirect 307, got {other:?}"),
    }
    assert!(matches!(
        cfg.rewrites[3].kind,
        RewriteKind::UrlRewrite { .. }
    ));
}

#[test]
fn surge_header_rewrite_phase_prefix_and_replace_regex() {
    let content = "\
[Header Rewrite]
http-request ^https://a\\.com/ header-del if-none-match
http-response ^https://a\\.com/ header-replace X-Cache hit
^https://b\\.com/ header-replace-regex X-Env prod.* stage
";
    let cfg = parse_import(content, ScriptDialect::Surge).unwrap();
    assert_eq!(cfg.rewrites.len(), 3);
    match &cfg.rewrites[0].kind {
        RewriteKind::HeaderRewrite { phase, name, .. } => {
            assert_eq!(*phase, Phase::Request);
            assert_eq!(name, "if-none-match");
        }
        other => panic!("expected HeaderRewrite, got {other:?}"),
    }
    match &cfg.rewrites[1].kind {
        RewriteKind::HeaderRewrite { phase, name, value } => {
            assert_eq!(*phase, Phase::Response);
            assert_eq!(name, "X-Cache");
            assert_eq!(value.as_deref(), Some("hit"));
        }
        other => panic!("expected HeaderRewrite, got {other:?}"),
    }
    assert!(matches!(
        cfg.rewrites[2].kind,
        RewriteKind::HeaderValueRewrite { .. }
    ));
}

#[test]
fn surge_body_rewrite_section_parsed() {
    let content = "\
[Body Rewrite]
http-response ^https://api\\.com/user \"vip\":false \"vip\":true
http-request ^https://api\\.com/up token=\\w+ token=redacted
";
    let cfg = parse_import(content, ScriptDialect::Surge).unwrap();
    assert_eq!(cfg.rewrites.len(), 2);
    match &cfg.rewrites[0].kind {
        RewriteKind::BodyRewrite {
            phase,
            body_pattern,
            replacement,
        } => {
            assert_eq!(*phase, Phase::Response);
            assert_eq!(body_pattern.as_ref().unwrap().as_str(), r#""vip":false"#);
            assert_eq!(replacement, r#""vip":true"#);
        }
        other => panic!("expected BodyRewrite, got {other:?}"),
    }
    match &cfg.rewrites[1].kind {
        RewriteKind::BodyRewrite {
            phase,
            body_pattern,
            replacement,
        } => {
            assert_eq!(*phase, Phase::Request);
            assert_eq!(body_pattern.as_ref().unwrap().as_str(), r"token=\w+");
            assert_eq!(replacement, "token=redacted");
        }
        other => panic!("expected BodyRewrite, got {other:?}"),
    }
}
