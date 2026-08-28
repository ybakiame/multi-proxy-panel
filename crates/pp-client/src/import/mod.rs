//! Third-party config snippet import (QX / Surge / Loon → pp-mitm / pp-script rules).
//!
//! This is the common foundation for "remote subscription (2.3b)" and "config import (2.4)":
//! parses Quantumult X / Surge / Loon rewrite / script / task / mitm config snippets into
//! pp-mitm [`RewriteRule`] / script hook rules and pp-script [`TaskScript`].
//!
//! Design trade-offs:
//! - Only covers commonly used subsets; unknown lines are skipped and recorded in
//!   [`ImportedConfig::warnings`] (comments / blank lines are silently skipped);
//! - Scripts are always treated as remote http(s) URLs: `source` is left empty, and the URL
//!   is recorded in [`ImportedConfig::script_urls`] / [`ImportedConfig::task_scripts`],
//!   to be fetched and backfilled by the caller;
//! - When pp-mitm fields are structurally incompatible with ecosystem syntax, existing fields
//!   are approximated and a warning is recorded (pp-mitm itself is not modified).

use pp_common::PanelResult;
use pp_mitm::{RewriteRule, ScriptRule as HookScriptRule};
use pp_script::{ScriptDialect, TaskScript};

#[cfg(test)]
use pp_mitm::Phase;
#[cfg(test)]
use pp_script::ScriptKind;

mod meta;
mod qx;
mod surge_loon;
mod utils;

pub use meta::*;
use qx::*;
use surge_loon::*;
use utils::*;

/// Result of a single import parse.
#[derive(Default)]
pub struct ImportedConfig {
    /// URL / Header / Body rewrite and Reject / Mock rules.
    pub rewrites: Vec<RewriteRule>,
    /// Script hook rules; `source` is empty, corresponding script URLs are in
    /// [`ImportedConfig::script_urls`].
    pub scripts: Vec<HookScriptRule>,
    /// Script hook remote addresses `(script_name, URL)`, corresponding to `scripts` in order.
    pub script_urls: Vec<(String, String)>,
    /// Scheduled tasks (`TaskScript.source` is empty) and their script URLs.
    pub task_scripts: Vec<(TaskScript, String)>,
    /// MITM hostname whitelist.
    pub hostnames: Vec<String>,
    /// Unrecognized lines / mapping deviations that cannot be expressed.
    pub warnings: Vec<String>,
    /// File header `#!key=value` metadata (see [`ConfigMeta`]).
    pub meta: ConfigMeta,
}

impl ImportedConfig {
    /// Record a warning: `tracing::warn` while also writing to the `warnings` list.
    fn warn(&mut self, section: &str, line: &str, msg: &str) {
        let text = format!("[{section}] {msg}: {line}");
        tracing::warn!(section, "{text}");
        self.warnings.push(text);
    }
}

/// Parse QX / Surge / Loon config snippets.
///
/// `dialect` is specified by the caller to indicate the source software, which determines
/// [`TaskScript::dialect`] and other dialect markers; syntactically isomorphic grammars
/// (e.g., Surge / Loon `[Script]`) share the same parsing path.
pub fn parse_import(content: &str, dialect: ScriptDialect) -> PanelResult<ImportedConfig> {
    let mut cfg = ImportedConfig {
        meta: parse_config_meta(content),
        ..ImportedConfig::default()
    };
    let mut section = String::new();
    let mut hook_index = 0usize;

    for raw in content.lines() {
        let line = raw.trim();
        if is_comment_or_blank(line) {
            continue;
        }
        if let Some(name) = section_name(line) {
            section = name;
            continue;
        }
        match section.as_str() {
            "rewrite_local" | "rewrite_remote" => {
                parse_qx_rewrite(&mut cfg, &mut hook_index, line);
            }
            "task_local" => parse_qx_task(&mut cfg, dialect, line),
            "mitm" => parse_mitm_hostnames(&mut cfg, line),
            "script" => {
                // Loon `[Script]` lines start with `http-request|http-response`
                // (Surge uses `name = type=...`); determine type by first token to avoid
                // dialect confusion.
                let first = line.split_whitespace().next().unwrap_or_default();
                if matches!(first, "http-request" | "http-response") {
                    parse_loon_script(&mut cfg, line);
                } else {
                    parse_surge_script(&mut cfg, dialect, line);
                }
            }
            "argument" => parse_loon_argument(&mut cfg, line),
            "url rewrite" => parse_surge_url_rewrite(&mut cfg, line),
            "header rewrite" => parse_surge_header_rewrite(&mut cfg, line),
            "map local" => parse_surge_map_local(&mut cfg, line),
            // Other sections (e.g., QX `[task_remote]` / Surge `[General]`) are skipped entirely.
            _ => {}
        }
    }

    Ok(cfg)
}

/// `[mitm]` / `[MITM]` hostname line parsing: `hostname = a, b, -exclude`.
///
/// `-` / `!` prefixes are exclusions (normalized to `-`), kept in `cfg.hostnames`
/// alongside whitelist entries; downstream (`build_mitm_proxy` / core routing rules)
/// filters by prefix. Surge's `%APPEND%` prefix is stripped.
fn parse_mitm_hostnames(cfg: &mut ImportedConfig, line: &str) {
    let Some(eq) = line.find('=') else {
        cfg.warn(
            "mitm",
            line,
            "unrecognized line (expected 'hostname = ...')",
        );
        return;
    };
    for entry in line[eq + 1..].split(',') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let entry = entry
            .strip_prefix("%APPEND%")
            .map(str::trim)
            .unwrap_or(entry);
        if let Some(rest) = entry.strip_prefix('!') {
            cfg.hostnames.push(format!("-{rest}"));
        } else {
            cfg.hostnames.push(entry.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pp_mitm::RewriteKind;

    #[test]
    fn qx_rewrite_local_all_rule_types() {
        let content = r#"
# 注释
[rewrite_local]
^https?://example\.com/api/(.*) url-and-header https://cdn.example.com/api/$1
^https?://example\.com/redir url-307 https://target.example.com/
^https?://example\.com/old url-302 https://new.example.com/$1
^https?://example\.com/rsp script-response-body https://example.com/rsp.js
^https?://example\.com/req script-request-body https://example.com/req.js
^https?://example\.com/echo script-echo-response https://example.com/echo.js
^https?://example\.com/block reject
^https?://example\.com/block2 reject-200
^https?://example\.com/block3 reject-dict
^https?://example\.com/page url-response-body secret REDACTED
^https?://example\.com/hdr url-request-header token MASKED
"#;
        let cfg = parse_import(content, ScriptDialect::QuantumultX).unwrap();

        // 8 rewrites: UrlRewrite x3, Reject x3, BodyRewrite x2
        assert_eq!(cfg.rewrites.len(), 8);
        assert!(matches!(
            cfg.rewrites[0].kind,
            RewriteKind::UrlRewrite { .. }
        ));
        assert!(matches!(
            cfg.rewrites[1].kind,
            RewriteKind::UrlRewrite { .. }
        ));
        assert!(matches!(
            cfg.rewrites[2].kind,
            RewriteKind::UrlRewrite { .. }
        ));
        assert!(matches!(cfg.rewrites[3].kind, RewriteKind::Reject));
        assert!(matches!(cfg.rewrites[4].kind, RewriteKind::Reject));
        assert!(matches!(cfg.rewrites[5].kind, RewriteKind::Reject));
        assert!(matches!(
            cfg.rewrites[6].kind,
            RewriteKind::BodyRewrite {
                phase: Phase::Response,
                ..
            }
        ));
        assert!(matches!(
            cfg.rewrites[7].kind,
            RewriteKind::BodyRewrite {
                phase: Phase::Request,
                ..
            }
        ));
        match &cfg.rewrites[0].kind {
            RewriteKind::UrlRewrite { target } => {
                assert_eq!(target, "https://cdn.example.com/api/$1")
            }
            other => panic!("unexpected kind: {other:?}"),
        }
        match &cfg.rewrites[7].kind {
            RewriteKind::BodyRewrite { replacement, .. } => assert_eq!(replacement, "MASKED"),
            other => panic!("unexpected kind: {other:?}"),
        }

        // 3 script hooks
        assert_eq!(cfg.scripts.len(), 3);
        assert_eq!(cfg.script_urls.len(), 3);
        assert_eq!(cfg.scripts[0].kind, ScriptKind::HttpResponse);
        assert!(cfg.scripts[0].requires_body);
        assert_eq!(cfg.scripts[0].max_size, 131072);
        assert!(cfg.scripts[0].source.is_empty());
        assert_eq!(cfg.script_urls[0].0, "hook-0");
        assert_eq!(cfg.script_urls[0].1, "https://example.com/rsp.js");
        assert_eq!(cfg.scripts[1].kind, ScriptKind::HttpRequest);
        assert!(cfg.scripts[1].requires_body);
        assert_eq!(cfg.script_urls[1].1, "https://example.com/req.js");
        assert_eq!(cfg.scripts[2].kind, ScriptKind::HttpResponse);
        assert!(!cfg.scripts[2].requires_body);
        assert_eq!(cfg.script_urls[2].1, "https://example.com/echo.js");

        // 302/307 and BodyRewrite deviations are recorded
        assert!(!cfg.warnings.is_empty());
        assert!(cfg.warnings.iter().any(|w| w.contains("url-307")));
    }

    #[test]
    fn qx_task_local_and_mitm_hostnames() {
        let content = r#"
[task_local]
0 9 * * * https://example.com/sign.js, tag=每日签到, img-url=https://example.com/icon.png
30 12 * * * https://example.com/clean.js

[mitm]
hostname = *.example.com, api.example2.com, -exclude.example.com
"#;
        let cfg = parse_import(content, ScriptDialect::QuantumultX).unwrap();

        assert_eq!(cfg.task_scripts.len(), 2);
        let (task, url) = &cfg.task_scripts[0];
        assert_eq!(task.name, "每日签到");
        assert_eq!(task.cron_expr, "0 0 9 * * *");
        assert!(task.source.is_empty());
        assert!(task.enabled);
        assert_eq!(task.dialect, ScriptDialect::QuantumultX);
        assert_eq!(url, "https://example.com/sign.js");

        let (task2, url2) = &cfg.task_scripts[1];
        assert_eq!(task2.name, "clean"); // derived from URL when no tag
        assert_eq!(task2.cron_expr, "0 30 12 * * *");
        assert_eq!(url2, "https://example.com/clean.js");

        // `-` prefix exclusions are kept with prefix in whitelist, no warning
        assert_eq!(
            cfg.hostnames,
            vec![
                "*.example.com".to_string(),
                "api.example2.com".to_string(),
                "-exclude.example.com".to_string()
            ]
        );
        assert!(
            !cfg.warnings
                .iter()
                .any(|w| w.contains("exclude.example.com"))
        );
    }

    #[test]
    fn mitm_hostname_bang_prefix_normalized_to_dash() {
        let content = r#"
[mitm]
hostname = *.example.com, !exclude.example.com
"#;
        let cfg = parse_import(content, ScriptDialect::QuantumultX).unwrap();

        // `!` prefix normalized to `-` and kept in whitelist, no exclusion warning.
        assert_eq!(
            cfg.hostnames,
            vec![
                "*.example.com".to_string(),
                "-exclude.example.com".to_string()
            ]
        );
        assert!(
            !cfg.warnings
                .iter()
                .any(|w| w.contains("exclude.example.com"))
        );
    }

    #[test]
    fn surge_script_url_rewrite_header_rewrite_map_local_mitm() {
        let content = r#"
[Script]
json = type=http-response,pattern=^https://api\.example\.com/,script-path=https://example.com/json.js,requires-body=true,max-size=131072,timeout=10
req = type=http-request,pattern=^https://example\.com/,script-path=https://example.com/req.js
task = type=cron,cronexp="* * * * *",script-path=https://example.com/task.js

[URL Rewrite]
^https://old\.example\.com/ https://new.example.com/$1
^https://old2\.example\.com/ https://new2.example.com/ request-header

[Header Rewrite]
^https://example\.com/ header-replace X-Foo bar baz
^https://example\.com/ header-del X-Bar

[Map Local]
^https://example\.com/offline data="<h1>offline</h1>" data-type=text status-code=200

[MITM]
hostname = %APPEND% *.example.com, -exclude.example.com
"#;
        let cfg = parse_import(content, ScriptDialect::Surge).unwrap();

        // [Script] script hooks
        assert_eq!(cfg.scripts.len(), 2);
        assert_eq!(cfg.script_urls.len(), 2);
        assert_eq!(cfg.scripts[0].name, "json");
        assert_eq!(cfg.scripts[0].kind, ScriptKind::HttpResponse);
        assert!(cfg.scripts[0].requires_body);
        assert_eq!(cfg.scripts[0].max_size, 131072);
        assert_eq!(
            cfg.scripts[0].pattern.as_str(),
            "^https://api\\.example\\.com/"
        );
        assert!(cfg.scripts[0].source.is_empty());
        assert_eq!(
            cfg.script_urls[0],
            (
                "json".to_string(),
                "https://example.com/json.js".to_string()
            )
        );
        assert_eq!(cfg.scripts[1].kind, ScriptKind::HttpRequest);
        assert!(!cfg.scripts[1].requires_body);
        assert_eq!(cfg.scripts[1].max_size, 131072); // default when max-size not specified

        // [Script] cron → TaskScript (5-part cron padded to 6)
        assert_eq!(cfg.task_scripts.len(), 1);
        let (task, url) = &cfg.task_scripts[0];
        assert_eq!(task.name, "task");
        assert_eq!(task.cron_expr, "0 * * * * *");
        assert_eq!(task.dialect, ScriptDialect::Surge);
        assert!(task.enabled);
        assert!(task.source.is_empty());
        assert_eq!(url, "https://example.com/task.js");

        // [URL Rewrite] 2 rules
        match &cfg.rewrites[0].kind {
            RewriteKind::UrlRewrite { target } => {
                assert_eq!(target, "https://new.example.com/$1")
            }
            other => panic!("unexpected kind: {other:?}"),
        }
        assert!(matches!(
            cfg.rewrites[1].kind,
            RewriteKind::UrlRewrite { .. }
        ));

        // [Header Rewrite] 2 rules
        match &cfg.rewrites[2].kind {
            RewriteKind::HeaderRewrite {
                phase: Phase::Request,
                name,
                value,
            } => {
                assert_eq!(name, "X-Foo");
                assert_eq!(value.as_deref(), Some("bar baz"));
            }
            other => panic!("unexpected kind: {other:?}"),
        }
        match &cfg.rewrites[3].kind {
            RewriteKind::HeaderRewrite {
                phase: Phase::Request,
                name,
                value,
            } => {
                assert_eq!(name, "X-Bar");
                assert_eq!(value, &None);
            }
            other => panic!("unexpected kind: {other:?}"),
        }

        // [Map Local] → Mock
        match &cfg.rewrites[4].kind {
            RewriteKind::Mock {
                status,
                body,
                headers,
            } => {
                assert_eq!(*status, 200);
                assert_eq!(body, "<h1>offline</h1>");
                // data-type=text → Content-Type: text/plain (appended when no explicit header=)
                assert_eq!(
                    headers,
                    &vec![("Content-Type".to_string(), "text/plain".to_string())]
                );
            }
            other => panic!("unexpected kind: {other:?}"),
        }

        // [MITM]: %APPEND% stripped, - exclusions kept with prefix
        assert_eq!(
            cfg.hostnames,
            vec![
                "*.example.com".to_string(),
                "-exclude.example.com".to_string()
            ]
        );
        assert!(
            !cfg.warnings
                .iter()
                .any(|w| w.contains("exclude.example.com"))
        );
        assert!(cfg.warnings.iter().any(|w| w.contains("request-header")));
    }

    /// iQiyi real sample: `header="Content-Type:application/json"` and `data-type=text`
    /// coexist; explicit header= takes priority (no data-type / header related warning).
    #[test]
    fn map_local_iqiyi_sample_with_explicit_header() {
        let content = r#"
[Map Local]
^https?:\/\/iface2\.iqiyi\.com\/control\/3\.0\/init_proxy\? data-type=text data="{}" status-code=200 header="Content-Type:application/json"
"#;
        let cfg = parse_import(content, ScriptDialect::Surge).unwrap();

        assert_eq!(cfg.rewrites.len(), 1);
        match &cfg.rewrites[0].kind {
            RewriteKind::Mock {
                status,
                body,
                headers,
            } => {
                assert_eq!(*status, 200);
                assert_eq!(body, "{}");
                assert_eq!(
                    headers,
                    &vec![("Content-Type".to_string(), "application/json".to_string())]
                );
            }
            other => panic!("unexpected kind: {other:?}"),
        }
        assert!(
            cfg.warnings.is_empty(),
            "should not produce data-type/header warnings: {:?}",
            cfg.warnings
        );
    }

    /// `header=` without colon and unknown `data-type` both record warnings;
    /// repeatable `header=` collected in order;
    /// when `header=` explicitly specifies Content-Type, `data-type` mapping is not appended
    /// (explicit takes priority).
    #[test]
    fn map_local_invalid_header_and_unknown_data_type_warn() {
        let content = r#"
[Map Local]
^https?://example\.com/off data="x" data-type=text header="X-A:1" header="broken" header="Content-Type:text/html" status-code=418
^https?://example\.com/unknown data="y" data-type=magic
"#;
        let cfg = parse_import(content, ScriptDialect::Surge).unwrap();

        assert_eq!(cfg.rewrites.len(), 2);
        match &cfg.rewrites[0].kind {
            RewriteKind::Mock {
                status,
                body,
                headers,
            } => {
                assert_eq!(*status, 418);
                assert_eq!(body, "x");
                // explicit Content-Type takes priority: data-type=text text/plain not appended;
                // "broken" without colon is dropped.
                assert_eq!(
                    headers,
                    &vec![
                        ("X-A".to_string(), "1".to_string()),
                        ("Content-Type".to_string(), "text/html".to_string()),
                    ]
                );
            }
            other => panic!("unexpected kind: {other:?}"),
        }
        match &cfg.rewrites[1].kind {
            RewriteKind::Mock {
                status,
                body,
                headers,
            } => {
                assert_eq!(*status, 200);
                assert_eq!(body, "y");
                assert!(
                    headers.is_empty(),
                    "unknown data-type should not append Content-Type"
                );
            }
            other => panic!("unexpected kind: {other:?}"),
        }
        assert!(
            cfg.warnings
                .iter()
                .any(|w| w.contains("invalid header 'broken'")),
            "should record invalid header warning: {:?}",
            cfg.warnings
        );
        assert!(
            cfg.warnings
                .iter()
                .any(|w| w.contains("unknown data-type 'magic'")),
            "should record unknown data-type warning: {:?}",
            cfg.warnings
        );
    }

    #[test]
    fn skips_unknown_comment_blank_lines_with_warnings() {
        let content = r#"
[rewrite_local]
# 注释
; 也是注释

^https?://example\.com/ok url-and-header https://ok.example.com/
totally unknown line
also unknown
[task_local]
garbage
[mitm]
hostname = *.example.com
"#;
        let cfg = parse_import(content, ScriptDialect::QuantumultX).unwrap();

        assert_eq!(cfg.rewrites.len(), 1);
        assert_eq!(cfg.hostnames, vec!["*.example.com".to_string()]);
        assert_eq!(cfg.task_scripts.len(), 0);
        // Unknown lines and unparseable task lines are recorded in warnings;
        // comments / blank lines are silently skipped
        assert_eq!(cfg.warnings.len(), 3);
    }

    #[test]
    fn invalid_regex_skipped_with_warning() {
        let content = r#"
[rewrite_local]
( url-and-header https://broken.example.com/
^https?://ok\.example\.com/ url-and-header https://ok2.example.com/

[Script]
bad = type=http-response,pattern=(,script-path=https://example.com/bad.js
"#;
        let cfg = parse_import(content, ScriptDialect::Surge).unwrap();

        // Invalid regex records 1 warning each in rewrite and script, no panic
        assert_eq!(cfg.rewrites.len(), 1);
        assert_eq!(cfg.scripts.len(), 0);
        assert_eq!(
            cfg.warnings
                .iter()
                .filter(|w| w.contains("invalid regex"))
                .count(),
            2
        );
    }

    #[test]
    fn parse_config_meta_extracts_header_fields_and_stops_at_section() {
        let content = r#"#!name=扫描全能王-解锁VIP
#!desc=扫描全能王-手机扫描仪 解锁黄金会员
#!date=2026-01-21
#!category=🐹 BOBO Premium
#!author=叮当猫chxm1023[https://github.com/chxm1023/Rewrite]
#!icon=https://example.com/CamScanner.png
#!openUrl=https://apps.apple.com/app/id388627783

[Script]
rule = type=http-response,pattern=^https://api-cs\.intsig\.net/,script-path=https://example.com/camscanner.js
"#;
        let meta = parse_config_meta(content);
        assert_eq!(meta.name.as_deref(), Some("扫描全能王-解锁VIP"));
        assert_eq!(
            meta.desc.as_deref(),
            Some("扫描全能王-手机扫描仪 解锁黄金会员")
        );
        assert_eq!(meta.date.as_deref(), Some("2026-01-21"));
        assert_eq!(meta.category.as_deref(), Some("🐹 BOBO Premium"));
        assert_eq!(
            meta.author.as_deref(),
            Some("叮当猫chxm1023[https://github.com/chxm1023/Rewrite]")
        );
        assert_eq!(
            meta.icon.as_deref(),
            Some("https://example.com/CamScanner.png")
        );
        assert_eq!(
            meta.open_url.as_deref(),
            Some("https://apps.apple.com/app/id388627783")
        );

        // parse_import also backfills meta
        let cfg = parse_import(content, ScriptDialect::Surge).unwrap();
        assert_eq!(cfg.meta.name.as_deref(), Some("扫描全能王-解锁VIP"));
        assert_eq!(
            cfg.meta.open_url.as_deref(),
            Some("https://apps.apple.com/app/id388627783")
        );
    }

    #[test]
    fn parse_config_meta_returns_all_none_without_header() {
        let content = r#"[rewrite_local]
^https?://example\.com/ url-and-header https://target.example.com/
"#;
        let meta = parse_config_meta(content);
        assert_eq!(meta, ConfigMeta::default());
        assert!(meta.name.is_none());
        assert!(meta.desc.is_none());
        assert!(meta.author.is_none());
        assert!(meta.icon.is_none());
        assert!(meta.date.is_none());
        assert!(meta.category.is_none());
        assert!(meta.open_url.is_none());
    }

    #[test]
    fn parse_config_meta_stops_at_first_non_bang_line() {
        // `#` comment (not `#!`) immediately stops header parsing
        let content = "#!name=有效名称\n# 普通注释\n#!desc=不应被解析\n";
        let meta = parse_config_meta(content);
        assert_eq!(meta.name.as_deref(), Some("有效名称"));
        assert!(meta.desc.is_none());
        // Empty line also stops
        let content2 = "#!name=A\n\n#!desc=B\n";
        let meta2 = parse_config_meta(content2);
        assert_eq!(meta2.name.as_deref(), Some("A"));
        assert!(meta2.desc.is_none());
    }

    /// `#!description=` is an alias for `#!desc=`: single key parses to `desc`.
    #[test]
    fn parse_config_meta_accepts_description_alias() {
        let content = "#!name=Demo\n#!description=使用说明\n";
        let meta = parse_config_meta(content);
        assert_eq!(meta.desc.as_deref(), Some("使用说明"));

        // Consistent through parse_import (sniff/import/cache all share parse_config_meta).
        let cfg = parse_import(content, ScriptDialect::Surge).unwrap();
        assert_eq!(cfg.meta.desc.as_deref(), Some("使用说明"));
    }

    /// `#!desc=` and `#!description=` coexist: `#!desc=` takes priority, alias does not override.
    #[test]
    fn parse_config_meta_description_alias_does_not_override_desc() {
        // desc first, description after: alias does not override.
        let content = "#!name=Demo\n#!desc=短描述\n#!description=全拼描述\n";
        let meta = parse_config_meta(content);
        assert_eq!(meta.desc.as_deref(), Some("短描述"));

        // description first, desc after: canonical key still takes effect (desc优先).
        let content2 = "#!name=Demo\n#!description=全拼描述\n#!desc=短描述\n";
        let meta2 = parse_config_meta(content2);
        assert_eq!(meta2.desc.as_deref(), Some("短描述"));
    }

    /// BaiDuTieBa sample regression: `#!desc=` value contains semicolon and Chinese,
    /// alias change does not interfere with canonical key parsing.
    #[test]
    fn parse_config_meta_baidu_tieba_desc_semicolon_chinese() {
        let content = "#!name=百度贴吧\n#!desc=开屏广告;推荐和吧内帖子列表的直播及广告\n";
        let meta = parse_config_meta(content);
        assert_eq!(
            meta.desc.as_deref(),
            Some("开屏广告;推荐和吧内帖子列表的直播及广告")
        );

        let cfg = parse_import(content, ScriptDialect::Surge).unwrap();
        assert_eq!(
            cfg.meta.desc.as_deref(),
            Some("开屏广告;推荐和吧内帖子列表的直播及广告")
        );
    }

    /// ① `#!arguments=` / `#!arguments-desc=` parsing: key/default-value/description merged into ArgSpec.
    #[test]
    fn config_meta_parses_arguments_decl_and_strict_desc() {
        let content = r#"#!name=Demo
#!arguments= server:api.example.com, token:default-token
#!arguments-desc= {"server":"API 服务器", "token":"鉴权令牌"}
"#;
        let meta = parse_config_meta(content);
        assert_eq!(meta.arguments.len(), 2);
        let server = meta.arguments.iter().find(|a| a.key == "server").unwrap();
        assert_eq!(server.default_value, "api.example.com");
        assert_eq!(server.description.as_deref(), Some("API 服务器"));
        let token = meta.arguments.iter().find(|a| a.key == "token").unwrap();
        assert_eq!(token.default_value, "default-token");
        assert_eq!(token.description.as_deref(), Some("鉴权令牌"));
        // default value containing spaces: value is preserved as a whole
        let content2 = "#!arguments= server:api.example.com, greeting:hello world\n";
        let meta2 = parse_config_meta(content2);
        assert_eq!(
            meta2
                .arguments
                .iter()
                .find(|a| a.key == "greeting")
                .unwrap()
                .default_value,
            "hello world"
        );
    }

    /// ① Non-strict JSON desc syntax (key unquoted) uses loose extraction;
    /// missing declaration keys are padded with empty default values.
    #[test]
    fn config_meta_parses_arguments_desc_loosely() {
        let content = "#!arguments= server:api.example.com\n#!arguments-desc= {server:\"API 服务器\", token: '鉴权 令牌'}\n";
        let meta = parse_config_meta(content);
        let server = meta.arguments.iter().find(|a| a.key == "server").unwrap();
        assert_eq!(server.default_value, "api.example.com");
        assert_eq!(server.description.as_deref(), Some("API 服务器"));
        // Key only in desc: empty default value padded into ArgSpec.
        let token = meta.arguments.iter().find(|a| a.key == "token").unwrap();
        assert_eq!(token.default_value, "");
        assert_eq!(token.description.as_deref(), Some("鉴权 令牌"));
    }

    /// ② `[Script]` line `argument=` parameter extracted into ScriptRule.argument.
    #[test]
    fn surge_script_line_extracts_argument_into_script_rule() {
        let content = r#"#!name=Demo
#!arguments= server:api.example.com, token:default-token
[Script]
xxx = type=http-response, pattern=^https://api\.example\.com/, script-path=https://example.com/json.js, argument={server}|{token}
plain = type=http-request, pattern=^https://example\.com/, script-path=https://example.com/req.js
"#;
        let cfg = parse_import(content, ScriptDialect::Surge).unwrap();
        assert_eq!(cfg.scripts.len(), 2);
        assert_eq!(cfg.scripts[0].argument.as_deref(), Some("{server}|{token}"));
        // Script without declared argument keeps None.
        assert_eq!(cfg.scripts[1].argument, None);
        // meta also parses arguments.
        assert_eq!(cfg.meta.arguments.len(), 2);
    }

    /// ① `#!arguments-desc=` naive syntax `key:description` (no `{}`, no quotes, can contain
    /// Chinese/spaces, comma-separated multiple) → ArgSpec.description correctly filled.
    #[test]
    fn config_meta_parses_arguments_desc_naive_syntax() {
        // Single naive syntax (BaiDuTieBa real sample).
        let content = r#"#!arguments=per_filter_video:0
#!arguments-desc=per_filter_video:设置为1则推荐页不展示视频贴
"#;
        let meta = parse_config_meta(content);
        assert_eq!(meta.arguments.len(), 1);
        let spec = &meta.arguments[0];
        assert_eq!(spec.key, "per_filter_video");
        assert_eq!(spec.default_value, "0");
        assert_eq!(
            spec.description.as_deref(),
            Some("设置为1则推荐页不展示视频贴")
        );

        // Multiple comma-separated: commas inside description are not mistakenly split,
        // only split at `,key:`.
        let content2 = r#"#!arguments=per_filter_video:0, banner:1
#!arguments-desc=per_filter_video:推荐页关闭视频贴, banner:顶部横幅开关
"#;
        let meta2 = parse_config_meta(content2);
        assert_eq!(meta2.arguments.len(), 2);
        let pf = meta2
            .arguments
            .iter()
            .find(|a| a.key == "per_filter_video")
            .unwrap();
        assert_eq!(pf.default_value, "0");
        assert_eq!(pf.description.as_deref(), Some("推荐页关闭视频贴"));
        let banner = meta2.arguments.iter().find(|a| a.key == "banner").unwrap();
        assert_eq!(banner.default_value, "1");
        assert_eq!(banner.description.as_deref(), Some("顶部横幅开关"));

        // Description containing comma (not `key:` prefix) is not split.
        let content3 = "#!arguments-desc=per_filter_video:推荐页关闭视频,弹窗\n";
        let meta3 = parse_config_meta(content3);
        assert_eq!(
            meta3.arguments[0].description.as_deref(),
            Some("推荐页关闭视频,弹窗")
        );
    }

    /// ③ Surge `[Script]` line `requires-body` numeric form `1/0` and boolean `true/false`
    /// both parse correctly.
    #[test]
    fn surge_script_requires_body_accepts_numeric_and_boolean() {
        let content = r#"
[Script]
num1 = type=http-response,pattern=^https://a\.com/,script-path=https://example.com/a.js,requires-body=1
num0 = type=http-response,pattern=^https://b\.com/,script-path=https://example.com/b.js,requires-body=0
bt = type=http-response,pattern=^https://c\.com/,script-path=https://example.com/c.js,requires-body=true
bf = type=http-response,pattern=^https://d\.com/,script-path=https://example.com/d.js,requires-body=false
none = type=http-response,pattern=^https://e\.com/,script-path=https://example.com/e.js
"#;
        let cfg = parse_import(content, ScriptDialect::Surge).unwrap();
        assert_eq!(cfg.scripts.len(), 5);
        assert!(cfg.scripts[0].requires_body, "requires-body=1 → true");
        assert!(!cfg.scripts[1].requires_body, "requires-body=0 → false");
        assert!(cfg.scripts[2].requires_body, "requires-body=true → true");
        assert!(!cfg.scripts[3].requires_body, "requires-body=false → false");
        assert!(!cfg.scripts[4].requires_body, "default → false");
    }

    /// ④ Surge `max-size=-1` / `0` (unlimited) mapped to 10MB upper bound;
    /// normal numbers parsed as-is; default falls back to 131072.
    #[test]
    fn surge_script_max_size_unlimited_and_normal_values() {
        let content = r#"
[Script]
unlimited = type=http-response,pattern=^https://a\.com/,script-path=https://example.com/a.js,max-size=-1
zero = type=http-response,pattern=^https://b\.com/,script-path=https://example.com/b.js,max-size=0
normal = type=http-response,pattern=^https://c\.com/,script-path=https://example.com/c.js,max-size=4096
default = type=http-response,pattern=^https://d\.com/,script-path=https://example.com/d.js
"#;
        let cfg = parse_import(content, ScriptDialect::Surge).unwrap();
        assert_eq!(cfg.scripts.len(), 4);
        assert_eq!(
            cfg.scripts[0].max_size,
            10 * 1024 * 1024,
            "max-size=-1 → unlimited"
        );
        assert_eq!(
            cfg.scripts[1].max_size,
            10 * 1024 * 1024,
            "max-size=0 → unlimited"
        );
        assert_eq!(cfg.scripts[2].max_size, 4096, "normal number as-is");
        assert_eq!(cfg.scripts[3].max_size, 131072, "default fallback");
    }

    /// ①③④ BaiDuTieBa.sgmodule real sample full assertion: naive desc, triple-brace
    /// placeholder preserved as-is, `requires-body=1`, `max-size=-1`, and pattern `\/(...)`
    /// all parse correctly.
    #[test]
    fn surge_script_badubatieba_sample_fixture() {
        let content = r#"#!arguments=per_filter_video:0
#!arguments-desc=per_filter_video:设置为1则推荐页不展示视频贴
[Script]
贴吧proto = type=http-response,pattern=^https?:\/\/(tiebac|c\.tieba)\.baidu\.com\/...$ ,requires-body=1,binary-body-mode=1,max-size=-1,script-path=https://example.com/x.js,argument=per_filter_video_thread={{{per_filter_video}}}
"#;
        let cfg = parse_import(content, ScriptDialect::Surge).unwrap();

        // Header parameters: naive desc merged into ArgSpec.
        assert_eq!(cfg.meta.arguments.len(), 1);
        let spec = &cfg.meta.arguments[0];
        assert_eq!(spec.key, "per_filter_video");
        assert_eq!(spec.default_value, "0");
        assert_eq!(
            spec.description.as_deref(),
            Some("设置为1则推荐页不展示视频贴")
        );

        // [Script] line: requires-body=1 / max-size=-1 / argument triple-brace placeholder preserved.
        assert_eq!(cfg.scripts.len(), 1);
        assert_eq!(cfg.scripts[0].name, "贴吧proto");
        assert_eq!(
            cfg.scripts[0].pattern.as_str(),
            r"^https?:\/\/(tiebac|c\.tieba)\.baidu\.com\/...$"
        );
        assert!(cfg.scripts[0].requires_body);
        assert_eq!(cfg.scripts[0].max_size, 10 * 1024 * 1024);
        assert_eq!(
            cfg.scripts[0].argument.as_deref(),
            Some("per_filter_video_thread={{{per_filter_video}}}")
        );
        assert_eq!(cfg.script_urls[0].1, "https://example.com/x.js");
    }

    /// ② QX rewrite script lines also extract `argument=`.
    #[test]
    fn qx_rewrite_script_line_extracts_argument() {
        let content = r#"
[rewrite_local]
^https?://api\.example\.com/(.*) script-response-body https://example.com/rsp.js argument={server}
"#;
        let cfg = parse_import(content, ScriptDialect::QuantumultX).unwrap();
        assert_eq!(cfg.scripts.len(), 1);
        assert_eq!(cfg.scripts[0].argument.as_deref(), Some("{server}"));
    }

    /// ① `#!` header `=` with whitespace around it: key trim-matched, value trim.
    #[test]
    fn parse_config_meta_trims_whitespace_around_equals() {
        let content = "#!name = 测试名称 \n#!desc = 描述内容\n#!author =  作者  \n#!icon = https://example.com/i.png\n";
        let meta = parse_config_meta(content);
        assert_eq!(meta.name.as_deref(), Some("测试名称"));
        assert_eq!(meta.desc.as_deref(), Some("描述内容"));
        assert_eq!(meta.author.as_deref(), Some("作者"));
        assert_eq!(meta.icon.as_deref(), Some("https://example.com/i.png"));
    }

    /// ② `#!arguments` quote-aware split: `Types:"Translate,External"` is not chopped,
    /// values are unquoted, keys with `[0]` subscript work normally.
    #[test]
    fn parse_arguments_decl_quote_aware_split_and_unquote() {
        let content = "#!arguments = Types:\"Translate,External\",Languages[0]:\"AUTO\",Vendor:\"Google\",plain:value\n";
        let meta = parse_config_meta(content);
        assert_eq!(meta.arguments.len(), 4);
        let types = meta.arguments.iter().find(|a| a.key == "Types").unwrap();
        assert_eq!(types.default_value, "Translate,External");
        let lang = meta
            .arguments
            .iter()
            .find(|a| a.key == "Languages[0]")
            .unwrap();
        assert_eq!(lang.default_value, "AUTO");
        let vendor = meta.arguments.iter().find(|a| a.key == "Vendor").unwrap();
        assert_eq!(vendor.default_value, "Google");
        // Unquoted bare value parsed as-is (regression of old syntax).
        let plain = meta.arguments.iter().find(|a| a.key == "plain").unwrap();
        assert_eq!(plain.default_value, "value");
    }

    /// ③ Loon `[Argument]` section parsing: kind (input/select) + options + tag + desc.
    #[test]
    fn loon_argument_section_parses_kind_options_tag_desc() {
        let content = r#"#!name=参数模块
[Argument]
Types = input,"Translate,External",tag=[歌词] 启用类型（多选）,desc=请选择要添加的歌词选项。
Languages[0] = select,"AUTO","ZH","ZH-HANS","EN",tag=[翻译器] 主语言,desc=仅当源语言识别不准确时更改。
"#;
        let cfg = parse_import(content, ScriptDialect::Loon).unwrap();
        assert_eq!(cfg.meta.arguments.len(), 2);
        let types = cfg
            .meta
            .arguments
            .iter()
            .find(|a| a.key == "Types")
            .unwrap();
        assert_eq!(types.kind, ArgKind::Input);
        assert_eq!(types.default_value, "Translate,External");
        assert!(types.options.is_empty());
        assert_eq!(types.tag.as_deref(), Some("[歌词] 启用类型（多选）"));
        assert_eq!(
            types.description.as_deref(),
            Some("请选择要添加的歌词选项。")
        );
        let lang = cfg
            .meta
            .arguments
            .iter()
            .find(|a| a.key == "Languages[0]")
            .unwrap();
        assert_eq!(lang.kind, ArgKind::Select);
        assert_eq!(lang.default_value, "AUTO");
        assert_eq!(lang.options, vec!["ZH", "ZH-HANS", "EN"]);
        assert_eq!(lang.tag.as_deref(), Some("[翻译器] 主语言"));
        assert_eq!(
            lang.description.as_deref(),
            Some("仅当源语言识别不准确时更改。")
        );
    }

    /// ④ Loon `[Script]` line format: `http-request|http-response ^pattern param=value,...`.
    #[test]
    fn loon_script_lines_parse_type_pattern_tag_argument() {
        let content = r#"#!name=DualSubs
[Script]
http-response ^https?:\/\/api\.spotify\.com\/v1\/tracks\? requires-body=1, script-path=https://example.com/r.js, tag=🍿️ DualSubs.Spotify.Tracks, argument=[{Types},{Languages[0]},{Vendor}]
http-request ^https?:\/\/spclient\.wg\.spotify\.com(:443)?\/color-lyrics\/v2\/track\/\w+\?(.*) requires-body=1, binary-body-mode=1, script-path=https://example.com/q.js, tag=req, argument=[{Types}]
"#;
        let cfg = parse_import(content, ScriptDialect::Loon).unwrap();
        assert_eq!(cfg.scripts.len(), 2);
        assert_eq!(cfg.scripts[0].name, "🍿️ DualSubs.Spotify.Tracks");
        assert_eq!(cfg.scripts[0].kind, ScriptKind::HttpResponse);
        assert_eq!(
            cfg.scripts[0].pattern.as_str(),
            r"^https?:\/\/api\.spotify\.com\/v1\/tracks\?"
        );
        assert!(cfg.scripts[0].requires_body);
        // `argument=` preserved as-is (contains `[{key},...]` template; commas/braces not mistakenly split).
        assert_eq!(
            cfg.scripts[0].argument.as_deref(),
            Some("[{Types},{Languages[0]},{Vendor}]")
        );
        assert_eq!(cfg.script_urls[0].1, "https://example.com/r.js");
        // Second script: http-request + tag naming + binary-body-mode ignored.
        assert_eq!(cfg.scripts[1].name, "req");
        assert_eq!(cfg.scripts[1].kind, ScriptKind::HttpRequest);
        assert!(cfg.scripts[1].requires_body);
        assert_eq!(cfg.scripts[1].argument.as_deref(), Some("[{Types}]"));
        assert_eq!(cfg.script_urls[1].1, "https://example.com/q.js");
        assert!(cfg.warnings.is_empty(), "warnings: {:?}", cfg.warnings);
    }

    /// ①②③④ DualSubs.Spotify Loon `.plugin` real sample full assertion.
    #[test]
    fn dualsubs_spotify_loon_plugin_sample_fixture() {
        let content = r#"#!name = 🍿️ DualSubs: 🎵 Spotify
#!desc = Spotify 增强及双语歌词
[Argument]
Types = input,"Translate,External",tag=[歌词] 启用类型（多选）,desc=请选择要添加的歌词选项。
Languages[0] = select,"AUTO","ZH","ZH-HANS","EN",tag=[翻译器] 主语言,desc=仅当源语言识别不准确时更改。
[Script]
http-response ^https?:\/\/api\.spotify\.com\/v1\/tracks\? requires-body=1, script-path=https://example.com/r.js, tag=🍿️ DualSubs.Spotify.Tracks, argument=[{Types},{Languages[0]},{Vendor}]
http-request ^https?:\/\/spclient\.wg\.spotify\.com(:443)?\/color-lyrics\/v2\/track\/\w+\?(.*) requires-body=1, binary-body-mode=1, script-path=https://example.com/q.js, tag=req, argument=[{Types}]
[MITM]
hostname = api.spotify.com, spclient.wg.spotify.com, *-spclient.spotify.com
"#;
        let cfg = parse_import(content, ScriptDialect::Loon).unwrap();

        // ① Header whitespace trim.
        assert_eq!(cfg.meta.name.as_deref(), Some("🍿️ DualSubs: 🎵 Spotify"));
        assert_eq!(cfg.meta.desc.as_deref(), Some("Spotify 增强及双语歌词"));

        // ③ [Argument]: input/select + options + tag + desc.
        assert_eq!(cfg.meta.arguments.len(), 2);
        let types = cfg
            .meta
            .arguments
            .iter()
            .find(|a| a.key == "Types")
            .unwrap();
        assert_eq!(types.kind, ArgKind::Input);
        assert_eq!(types.default_value, "Translate,External");
        assert_eq!(types.tag.as_deref(), Some("[歌词] 启用类型（多选）"));
        let lang = cfg
            .meta
            .arguments
            .iter()
            .find(|a| a.key == "Languages[0]")
            .unwrap();
        assert_eq!(lang.kind, ArgKind::Select);
        assert_eq!(lang.default_value, "AUTO");
        assert_eq!(lang.options, vec!["ZH", "ZH-HANS", "EN"]);

        // ④ [Script]: two Loon lines.
        assert_eq!(cfg.scripts.len(), 2);
        assert_eq!(cfg.scripts[0].name, "🍿️ DualSubs.Spotify.Tracks");
        assert_eq!(cfg.scripts[0].kind, ScriptKind::HttpResponse);
        assert!(cfg.scripts[0].requires_body);
        assert_eq!(
            cfg.scripts[0].argument.as_deref(),
            Some("[{Types},{Languages[0]},{Vendor}]")
        );
        assert_eq!(cfg.script_urls[0].1, "https://example.com/r.js");
        assert_eq!(cfg.scripts[1].name, "req");
        assert_eq!(cfg.scripts[1].kind, ScriptKind::HttpRequest);
        assert_eq!(cfg.script_urls[1].1, "https://example.com/q.js");

        // [MITM] hostname whitelist.
        assert_eq!(
            cfg.hostnames,
            vec![
                "api.spotify.com".to_string(),
                "spclient.wg.spotify.com".to_string(),
                "*-spclient.spotify.com".to_string()
            ]
        );
        assert!(cfg.warnings.is_empty(), "warnings: {:?}", cfg.warnings);
    }

    /// ①② Surge `.sgmodule` sample: `#!arguments` quote-aware split + Surge `[Script]` line
    /// `{{{key}}}` placeholder preserved as-is and `engine` ignored.
    #[test]
    fn dualsubs_spotify_surge_sgmodule_sample_fixture() {
        let content = r#"#!name = 🍿️ DualSubs: 🎵 Spotify
#!desc = Spotify 增强及双语歌词
#!arguments = Types:"Translate,External",Languages[0]:"AUTO",Languages[1]:"ZH",Vendor:"Google",LogLevel:"WARN"
[Script]
🍿️ DualSubs.Spotify.Tracks = type=http-response, pattern=^https?:\/\/api\.spotify\.com\/v1\/tracks\?, requires-body=1, engine=webview, script-path=https://example.com/r.js, argument=Types="{{{Types}}}"&Languages[0]="{{{Languages[0]}}}"&Vendor="{{{Vendor}}}"
"#;
        let cfg = parse_import(content, ScriptDialect::Surge).unwrap();

        // ① Header whitespace trim.
        assert_eq!(cfg.meta.name.as_deref(), Some("🍿️ DualSubs: 🎵 Spotify"));
        assert_eq!(cfg.meta.desc.as_deref(), Some("Spotify 增强及双语歌词"));

        // ② `#!arguments` quote-aware: comma-containing values not chopped, keys with `[0]`, unquoted.
        assert_eq!(cfg.meta.arguments.len(), 5);
        let types = cfg
            .meta
            .arguments
            .iter()
            .find(|a| a.key == "Types")
            .unwrap();
        assert_eq!(types.default_value, "Translate,External");
        let lang0 = cfg
            .meta
            .arguments
            .iter()
            .find(|a| a.key == "Languages[0]")
            .unwrap();
        assert_eq!(lang0.default_value, "AUTO");
        let lang1 = cfg
            .meta
            .arguments
            .iter()
            .find(|a| a.key == "Languages[1]")
            .unwrap();
        assert_eq!(lang1.default_value, "ZH");
        let vendor = cfg
            .meta
            .arguments
            .iter()
            .find(|a| a.key == "Vendor")
            .unwrap();
        assert_eq!(vendor.default_value, "Google");

        // Surge [Script]: name = type=...; `{{{key}}}` placeholder preserved, engine ignored.
        assert_eq!(cfg.scripts.len(), 1);
        assert_eq!(cfg.scripts[0].name, "🍿️ DualSubs.Spotify.Tracks");
        assert_eq!(cfg.scripts[0].kind, ScriptKind::HttpResponse);
        assert!(cfg.scripts[0].requires_body);
        assert_eq!(
            cfg.scripts[0].argument.as_deref(),
            Some(
                "Types=\"{{{Types}}}\"&Languages[0]=\"{{{Languages[0]}}}\"&Vendor=\"{{{Vendor}}}\""
            )
        );
        assert_eq!(cfg.script_urls[0].1, "https://example.com/r.js");
    }
}
