use super::*;
use crate::import::{ArgSpec, parse_import};
use pp_mitm::ScriptRule;
use pp_script::ScriptKind;
use regex::Regex;
use std::collections::HashMap;

/// `resolve_argument_template`: user values take priority -> defaults -> keep as-is.
#[test]
fn resolve_argument_template_substitutes_user_values_then_defaults() {
    let user_values = HashMap::from([("token".to_string(), "abc".to_string())]);
    let defaults = HashMap::from([("server".to_string(), "api.example.com".to_string())]);
    assert_eq!(
        resolve_argument_template("{server}|{token}", &user_values, &defaults),
        "api.example.com|abc"
    );
    // Undeclared placeholder kept as-is.
    assert_eq!(
        resolve_argument_template("{server}|{missing}", &user_values, &defaults),
        "api.example.com|{missing}"
    );
    // No user values, no defaults: kept as-is.
    assert_eq!(
        resolve_argument_template("{server}", &HashMap::new(), &HashMap::new()),
        "{server}"
    );
}

/// `apply_argument_templates`: user values take priority over defaults, undeclared placeholders kept.
#[test]
fn apply_argument_templates_prefers_user_values_over_defaults() {
    let rules = vec![
        ScriptRule {
            name: "r1".into(),
            kind: ScriptKind::HttpResponse,
            pattern: Regex::new(".*").unwrap(),
            requires_body: false,
            max_size: 131072,
            source: String::new(),
            argument: Some("{server}|{token}|{extra}".to_string()),
        },
        ScriptRule {
            name: "r2".into(),
            kind: ScriptKind::HttpRequest,
            pattern: Regex::new(".*").unwrap(),
            requires_body: false,
            max_size: 131072,
            source: String::new(),
            argument: None,
        },
    ];
    let metas = vec![crate::import::ConfigMeta {
        arguments: vec![
            ArgSpec {
                key: "server".into(),
                default_value: "api.example.com".into(),
                description: None,
                ..ArgSpec::default()
            },
            ArgSpec {
                key: "token".into(),
                default_value: "default-token".into(),
                description: None,
                ..ArgSpec::default()
            },
        ],
        ..crate::import::ConfigMeta::default()
    }];
    let remotes = vec![RemoteResource {
        argument_values: vec![("token".to_string(), "abc".to_string())],
        ..RemoteResource::default()
    }];

    let out = apply_argument_templates(rules, &metas, &remotes);
    assert_eq!(
        out[0].argument.as_deref(),
        Some("api.example.com|abc|{extra}")
    );
    // argument is None: kept as-is.
    assert_eq!(out[1].argument, None);
}

/// `resolve_argument_template` supports Surge standard triple-brace placeholder `{{{key}}}`
/// (prefer long form), also compatible with shorthand `{key}`; user values take priority over defaults,
/// undeclared placeholders kept as-is.
#[test]
fn resolve_argument_template_supports_triple_brace_placeholders() {
    let user_values = HashMap::from([("per_filter_video".to_string(), "1".to_string())]);
    let defaults = HashMap::from([("per_filter_video".to_string(), "0".to_string())]);

    // Triple-brace placeholder: no user value -> default 0.
    assert_eq!(
        resolve_argument_template(
            "per_filter_video_thread={{{per_filter_video}}}",
            &HashMap::new(),
            &defaults,
        ),
        "per_filter_video_thread=0"
    );
    // Triple-brace placeholder: user value overrides default.
    assert_eq!(
        resolve_argument_template(
            "per_filter_video_thread={{{per_filter_video}}}",
            &user_values,
            &defaults,
        ),
        "per_filter_video_thread=1"
    );
    // Long form priority: `{{{a}}}` replaced as a whole, not contaminated by shorthand `{a}`.
    assert_eq!(
        resolve_argument_template(
            "{{{a}}}|{a}",
            &HashMap::new(),
            &HashMap::from([("a".to_string(), "X".to_string())]),
        ),
        "X|X"
    );
    // Undeclared triple-brace placeholder kept as-is.
    assert_eq!(
        resolve_argument_template("{{{missing}}}", &HashMap::new(), &HashMap::new(),),
        "{{{missing}}}"
    );
    // Shorthand and triple-brace coexist.
    assert_eq!(
        resolve_argument_template(
            "{server}|{{{token}}}",
            &HashMap::from([("token".to_string(), "abc".to_string())]),
            &HashMap::from([("server".to_string(), "api.example.com".to_string())]),
        ),
        "api.example.com|abc"
    );
}

/// `resolve_argument_template` supports arbitrary keys (including `[0]` subscript) and Loon `[{key},...]`
/// template: entire argument string replaced by `{key}`, outer `[]` kept; Surge triple-brace also replaced.
#[test]
fn resolve_argument_template_supports_arbitrary_keys_and_bracket_forms() {
    let user_values = HashMap::from([
        ("Types".to_string(), "Translate,External".to_string()),
        ("Languages[0]".to_string(), "AUTO".to_string()),
    ]);
    let defaults = HashMap::new();

    // Loon `[{key}]` form: `{Types}` / `{Languages[0]}` replaced one by one, outer brackets kept;
    // undeclared `{Vendor}` kept as-is.
    assert_eq!(
        resolve_argument_template("[{Types},{Languages[0]},{Vendor}]", &user_values, &defaults,),
        "[Translate,External,AUTO,{Vendor}]"
    );

    // Surge triple-brace placeholder with array subscript key: replaced as a whole, inner quotes kept.
    assert_eq!(
        resolve_argument_template(
            "Types=\"{{{Types}}}\"&Languages[0]=\"{{{Languages[0]}}}\"&Vendor=\"{{{Vendor}}}\"",
            &user_values,
            &defaults,
        ),
        "Types=\"Translate,External\"&Languages[0]=\"AUTO\"&Vendor=\"{{{Vendor}}}\""
    );

    // Default fallback: no user value uses `#!arguments` default value.
    let with_defaults = HashMap::from([
        ("Types".to_string(), "External".to_string()),
        ("Languages[0]".to_string(), "ZH".to_string()),
    ]);
    assert_eq!(
        resolve_argument_template("[{Types},{Languages[0]}]", &HashMap::new(), &with_defaults),
        "[External,ZH]"
    );
}

/// DualSubs.Spotify full path assertion: Loon `.plugin` `[Argument]` + `[Script]`
/// via parse_import -> apply_argument_templates, `[{Types},{Languages[0]},{Vendor}]`
/// declared keys replaced, undeclared `{Vendor}` kept.
#[test]
fn dualsubs_spotify_loon_plugin_argument_template_resolution() {
    let content = r#"#!name = DualSubs: Spotify
[Argument]
Types = input,"Translate,External",tag=[Lyrics] Enable Type (Multi-select),desc=Please select lyrics options.
Languages[0] = select,"AUTO","ZH","ZH-HANS","EN",tag=[Translator] Primary Language,desc=Only change when source language recognition is inaccurate.
[Script]
http-response ^https?:\/\/api\.spotify\.com\/v1\/tracks\? requires-body=1, script-path=https://example.com/r.js, tag=DualSubs.Spotify.Tracks, argument=[{Types},{Languages[0]},{Vendor}]
"#;
    let imported = parse_import(content, ScriptDialect::Loon).unwrap();

    // Construct defaults from [Argument] (Types / Languages[0]).
    let metas = [imported.meta.clone()];
    let mut defaults = HashMap::new();
    for arg in &metas[0].arguments {
        defaults.insert(arg.key.clone(), arg.default_value.clone());
    }

    // No user value: Types and Languages[0] use defaults, {Vendor} kept.
    let resolved = resolve_argument_template(
        imported.scripts[0].argument.as_deref().unwrap(),
        &HashMap::new(),
        &defaults,
    );
    assert_eq!(resolved, "[Translate,External,AUTO,{Vendor}]");

    // User config value overrides default.
    let user = HashMap::from([
        ("Types".to_string(), "Translate".to_string()),
        ("Languages[0]".to_string(), "ZH".to_string()),
        ("Vendor".to_string(), "Google".to_string()),
    ]);
    let resolved2 = resolve_argument_template(
        imported.scripts[0].argument.as_deref().unwrap(),
        &user,
        &defaults,
    );
    assert_eq!(resolved2, "[Translate,ZH,Google]");
}

/// Surge `.sgmodule` `{{{key}}}` placeholder replacement (including `Languages[0]` subscript key).
#[test]
fn dualsubs_spotify_surge_sgmodule_argument_template_resolution() {
    let content = r#"#!name = DualSubs: Spotify
#!arguments = Types:"Translate,External",Languages[0]:"AUTO",Vendor:"Google"
[Script]
DualSubs.Spotify.Tracks = type=http-response, pattern=^https?:\/\/api\.spotify\.com\/v1\/tracks\?, requires-body=1, engine=webview, script-path=https://example.com/r.js, argument=Types="{{{Types}}}"&Languages[0]="{{{Languages[0]}}}"&Vendor="{{{Vendor}}}"
"#;
    let imported = parse_import(content, ScriptDialect::Surge).unwrap();
    let mut defaults = HashMap::new();
    for arg in &imported.meta.arguments {
        defaults.insert(arg.key.clone(), arg.default_value.clone());
    }

    let resolved = resolve_argument_template(
        imported.scripts[0].argument.as_deref().unwrap(),
        &HashMap::new(),
        &defaults,
    );
    assert_eq!(
        resolved,
        "Types=\"Translate,External\"&Languages[0]=\"AUTO\"&Vendor=\"Google\""
    );
}

/// `RemoteResource` new fields (argument_values / icon) preserved through save -> load roundtrip;
/// old manifests missing these fields use serde default.
#[test]
fn remote_resource_argument_values_and_icon_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let manager = RemoteManager::new(dir.path().to_path_buf());
    let remotes = vec![RemoteResource {
        name: "args".into(),
        url: "http://example.com/mod.sgmodule".into(),
        kind: RemoteKind::Snippet,
        dialect: ScriptDialect::Surge,
        argument_values: vec![
            ("server".to_string(), "api.example.com".to_string()),
            ("token".to_string(), "abc".to_string()),
        ],
        icon: Some("https://example.com/icon.png".to_string()),
        ..RemoteResource::default()
    }];
    manager.save(&remotes).unwrap();
    assert_eq!(manager.load().unwrap(), remotes);

    // Old manifest (no new fields) reads back to defaults, no error.
    std::fs::write(
                manager.remotes_file(),
                r#"[{"name":"old","url":"http://example.com/x.js","kind":"Script","dialect":"Surge","update_interval_secs":86400,"enabled":true}]"#,
            )
            .unwrap();
    let loaded = manager.load().unwrap();
    assert_eq!(loaded.len(), 1);
    assert!(loaded[0].argument_values.is_empty());
    assert_eq!(loaded[0].icon, None);
}

/// `prefill_argument_values`: prefill from parameter declaration defaults, existing user values not overwritten;
/// save -> load roundtrip preserves `arguments` / `argument_values`.
#[test]
fn prefill_argument_values_fills_defaults_and_roundtrips() {
    let mut remote = RemoteResource {
        name: "mod".into(),
        url: "http://example.com/mod.sgmodule".into(),
        kind: RemoteKind::Snippet,
        dialect: ScriptDialect::Surge,
        arguments: vec![
            ArgSpec {
                key: "server".into(),
                default_value: "api.example.com".into(),
                ..ArgSpec::default()
            },
            ArgSpec {
                key: "token".into(),
                default_value: "default-token".into(),
                ..ArgSpec::default()
            },
        ],
        argument_values: vec![("token".to_string(), "user-token".to_string())],
        ..RemoteResource::default()
    };
    prefill_argument_values(&mut remote);
    // server prefilled from default; token already has user value, not overwritten.
    assert_eq!(
        remote.argument_values,
        vec![
            ("token".to_string(), "user-token".to_string()),
            ("server".to_string(), "api.example.com".to_string()),
        ]
    );

    let dir = tempfile::tempdir().unwrap();
    let manager = RemoteManager::new(dir.path().to_path_buf());
    manager.save(&[remote.clone()]).unwrap();
    let loaded = manager.load().unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].arguments, remote.arguments);
    assert_eq!(loaded[0].argument_values, remote.argument_values);

    // Old manifest (no arguments field) deserializes back to empty list.
    std::fs::write(
                manager.remotes_file(),
                r#"[{"name":"old","url":"http://example.com/x.js","kind":"Script","dialect":"Surge","update_interval_secs":86400,"enabled":true}]"#,
            )
            .unwrap();
    let legacy = manager.load().unwrap();
    assert!(legacy[0].arguments.is_empty());
}

/// Fetch backfill (auxiliary path): resource declares no parameters but remote meta has declarations,
/// after fetch manifest backfills `arguments` and prefills `argument_values` from defaults.
#[tokio::test]
async fn fetch_backfills_arguments_and_defaults_when_resource_declares_none() {
    let base = spawn_remote_server(|base| {
                format!(
                    "#!name=Param Module\n\
                     #!arguments= server:api.example.com, token:default-token\n\
                     \n\
                     [Script]\n\
                     xxx = type=http-response, pattern=^https://api.example.com/, script-path={base}/hook.js, argument={{server}}|{{token}}\n"
                )
            })
            .await;
    let dir = tempfile::tempdir().unwrap();
    let manager = RemoteManager::new(dir.path().to_path_buf());
    let remotes = vec![RemoteResource {
        name: "args".into(),
        url: format!("{base}/snippet"),
        kind: RemoteKind::Snippet,
        dialect: ScriptDialect::Surge,
        ..RemoteResource::default()
    }];
    manager.save(&remotes).unwrap();

    let report = manager.fetch_all(&remotes).await;
    assert_eq!(report.fetched, 1);
    assert!(
        report.warnings.is_empty(),
        "warnings: {:?}",
        report.warnings
    );

    let loaded = manager.load().unwrap();
    assert_eq!(loaded[0].arguments.len(), 2);
    assert!(
        loaded[0]
            .argument_values
            .iter()
            .any(|(k, v)| k == "server" && v == "api.example.com")
    );
    assert!(
        loaded[0]
            .argument_values
            .iter()
            .any(|(k, v)| k == "token" && v == "default-token")
    );
}

/// `#!arguments` declaration and `argument=` template through fetch -> cache -> load roundtrip not lost,
/// meta exposed to `MergedRemoteConfig.metas`.
#[tokio::test]
async fn snippet_arguments_and_meta_roundtrip_through_cache() {
    let base = spawn_remote_server(|base| {
                format!(
                    "#!name=Param Module\n\
                     #!icon=https://example.com/icon.png\n\
                     #!arguments= server:api.example.com, token:default-token\n\
                     #!arguments-desc= {{server:\"API Server\", token:\"Auth Token\"}}\n\
                     \n\
                     [Script]\n\
                     xxx = type=http-response, pattern=^https://api.example.com/, script-path={base}/hook.js, argument={{server}}|{{token}}\n"
                )
            })
            .await;
    let dir = tempfile::tempdir().unwrap();
    let manager = RemoteManager::new(dir.path().to_path_buf());
    let remotes = vec![RemoteResource {
        name: "args".into(),
        url: format!("{base}/snippet"),
        kind: RemoteKind::Snippet,
        dialect: ScriptDialect::Surge,
        ..RemoteResource::default()
    }];

    let report = manager.fetch_all(&remotes).await;
    assert_eq!(report.fetched, 1);
    assert!(
        report.warnings.is_empty(),
        "warnings: {:?}",
        report.warnings
    );

    let merged = manager.load_cached().unwrap();
    assert_eq!(merged.scripts.len(), 1);
    assert_eq!(
        merged.scripts[0].argument.as_deref(),
        Some("{server}|{token}")
    );
    assert_eq!(merged.metas.len(), 1);
    let meta = &merged.metas[0];
    assert_eq!(meta.name.as_deref(), Some("Param Module"));
    assert_eq!(meta.icon.as_deref(), Some("https://example.com/icon.png"));
    assert_eq!(meta.arguments.len(), 2);
    let server = meta.arguments.iter().find(|a| a.key == "server").unwrap();
    assert_eq!(server.default_value, "api.example.com");
    assert_eq!(server.description.as_deref(), Some("API Server"));
}

/// BaiDuTieBa real sample through fetch -> cache -> load -> apply full path: plain desc,
/// numeric boolean requires-body, unlimited max-size, triple-brace placeholder replaced by default/user value.
#[tokio::test]
async fn snippet_badubatieba_sample_roundtrip_and_argument_resolution() {
    // `argument=` template contains triple-brace, use literal to avoid format escaping.
    let arg_tpl = "per_filter_video_thread={{{per_filter_video}}}";
    let base = spawn_remote_server(move |base| {
                format!(
                    "#!arguments=per_filter_video:0\n\
                     #!arguments-desc=per_filter_video:Set to 1 to hide video posts in recommendation\n\
                     \n\
                     [Script]\n\
                     TieBaProto = type=http-response,pattern=^https?:\\/\\/(tiebac|c\\.tieba)\\.baidu\\.com\\/...$ ,requires-body=1,binary-body-mode=1,max-size=-1,script-path={base}/hook.js,argument={arg_tpl}\n"
                )
            })
            .await;
    let dir = tempfile::tempdir().unwrap();
    let manager = RemoteManager::new(dir.path().to_path_buf());
    let remotes = vec![RemoteResource {
        name: "baidu".into(),
        url: format!("{base}/snippet"),
        kind: RemoteKind::Snippet,
        dialect: ScriptDialect::Surge,
        ..RemoteResource::default()
    }];

    let report = manager.fetch_all(&remotes).await;
    assert_eq!(report.fetched, 1);
    assert!(
        report.warnings.is_empty(),
        "warnings: {:?}",
        report.warnings
    );

    // meta: plain desc and default value merged into ArgSpec.
    let merged = manager.load_cached().unwrap();
    assert_eq!(merged.metas.len(), 1);
    let spec = &merged.metas[0].arguments[0];
    assert_eq!(spec.key, "per_filter_video");
    assert_eq!(spec.default_value, "0");
    assert_eq!(
        spec.description.as_deref(),
        Some("Set to 1 to hide video posts in recommendation")
    );

    // Script hook: requires-body=1 / max-size=-1 / triple-brace placeholder kept as-is.
    assert_eq!(merged.scripts.len(), 1);
    assert_eq!(merged.scripts[0].name, "TieBaProto");
    assert!(merged.scripts[0].requires_body);
    assert_eq!(merged.scripts[0].max_size, 10 * 1024 * 1024);
    assert_eq!(
        merged.scripts[0].argument.as_deref(),
        Some("per_filter_video_thread={{{per_filter_video}}}")
    );

    // apply_argument_templates: no user value -> default 0.
    let resolved = apply_argument_templates(merged.scripts, &merged.metas, &remotes);
    assert_eq!(
        resolved[0].argument.as_deref(),
        Some("per_filter_video_thread=0")
    );

    // User config value 1 -> overrides default.
    let remotes_with_value = vec![RemoteResource {
        name: "baidu".into(),
        url: format!("{base}/snippet"),
        kind: RemoteKind::Snippet,
        dialect: ScriptDialect::Surge,
        argument_values: vec![("per_filter_video".to_string(), "1".to_string())],
        ..RemoteResource::default()
    }];
    let merged2 = manager.load_cached().unwrap();
    let resolved2 = apply_argument_templates(merged2.scripts, &merged2.metas, &remotes_with_value);
    assert_eq!(
        resolved2[0].argument.as_deref(),
        Some("per_filter_video_thread=1")
    );
}

/// `update_resource`: full update by name, keep old cache; non-existent name errors.
#[test]
fn update_resource_updates_fields_and_keeps_old_cache() {
    let dir = tempfile::tempdir().unwrap();
    let manager = RemoteManager::new(dir.path().to_path_buf());
    let original = RemoteResource {
        name: "rules".into(),
        url: "http://a.example.com/r.conf".into(),
        kind: RemoteKind::Snippet,
        dialect: ScriptDialect::QuantumultX,
        ..RemoteResource::default()
    };
    manager.save(&[original]).unwrap();

    // Write old cache file before update, assert kept after update.
    let cache_dir = dir.path().join("remote_cache");
    std::fs::create_dir_all(&cache_dir).unwrap();
    std::fs::write(
        cache_dir.join("rules.json"),
        r#"{"rewrites":[],"scripts":[],"task_scripts":[],"hostnames":["*.example.com"]}"#,
    )
    .unwrap();

    let updated = RemoteResource {
        name: "rules".into(),
        url: "http://b.example.com/r2.conf".into(),
        kind: RemoteKind::Snippet,
        dialect: ScriptDialect::Surge,
        update_interval_secs: 3600,
        enabled: false,
        description: Some("desc".into()),
        argument_values: vec![("k".to_string(), "v".to_string())],
        icon: Some("http://i.example.com/x.png".into()),
        arguments: vec![ArgSpec {
            key: "k".into(),
            default_value: "v".into(),
            ..ArgSpec::default()
        }],
    };
    manager.update_resource("rules", updated).unwrap();

    let loaded = manager.load().unwrap();
    assert_eq!(loaded.len(), 1);
    let r = &loaded[0];
    assert_eq!(
        r.name, "rules",
        "name is the lookup key, unchanged after update"
    );
    assert_eq!(r.url, "http://b.example.com/r2.conf");
    assert_eq!(r.dialect, ScriptDialect::Surge);
    assert_eq!(r.update_interval_secs, 3600);
    assert!(!r.enabled);
    assert_eq!(r.description.as_deref(), Some("desc"));
    assert_eq!(r.argument_values, vec![("k".to_string(), "v".to_string())]);
    assert_eq!(r.icon.as_deref(), Some("http://i.example.com/x.png"));
    assert_eq!(r.arguments.len(), 1);
    assert_eq!(r.arguments[0].key, "k");
    // Old cache file kept.
    assert!(cache_dir.join("rules.json").exists());
}

#[test]
fn update_resource_returns_error_for_missing_name() {
    let dir = tempfile::tempdir().unwrap();
    let manager = RemoteManager::new(dir.path().to_path_buf());
    let err = manager
        .update_resource("missing", RemoteResource::default())
        .unwrap_err();
    assert!(err.to_string().contains("不存在"));
    assert!(manager.load().unwrap().is_empty());
}
