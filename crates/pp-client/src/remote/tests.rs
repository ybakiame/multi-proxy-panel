    use super::*;
    use crate::import::{ArgSpec, ImportedConfig, parse_import};
    use axum::http::StatusCode;
    use pp_mitm::{RewriteKind, ScriptRule};
    use pp_script::{ScriptDialect, ScriptKind};
    use regex::Regex;

    /// 启动本地 HTTP 服务（禁外部网络）：
    /// - `/snippet`：由 `snippet` 闭包基于服务地址生成片段内容
    /// - `/hook.js` / `/task.js` / `/script.js`：固定脚本内容
    /// - `/missing.js`：404
    async fn spawn_remote_server(snippet: impl Fn(&str) -> String) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base = format!("http://{addr}");
        let snippet = snippet(&base);
        let app = axum::Router::new()
            .route(
                "/snippet",
                axum::routing::get(move || async move { snippet }),
            )
            .route(
                "/hook.js",
                axum::routing::get(|| async { "const hook = 1;" }),
            )
            .route(
                "/task.js",
                axum::routing::get(|| async { "const task = 2;" }),
            )
            .route(
                "/script.js",
                axum::routing::get(|| async { "const script = 3;" }),
            )
            .route(
                "/missing.js",
                axum::routing::get(|| async { (StatusCode::NOT_FOUND, "not found") }),
            );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        base
    }

    #[test]
    fn save_load_roundtrip_applies_defaults_and_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let manager = RemoteManager::new(dir.path().to_path_buf());

        // 文件不存在 → 空列表
        assert!(manager.load().unwrap().is_empty());

        let remotes = vec![RemoteResource {
            name: "rules".into(),
            url: "http://example.com/rules.conf".into(),
            kind: RemoteKind::Snippet,
            dialect: ScriptDialect::QuantumultX,
            ..RemoteResource::default()
        }];
        manager.save(&remotes).unwrap();
        assert!(manager.remotes_file().exists());
        assert_eq!(manager.load().unwrap(), remotes);
    }

    #[tokio::test]
    async fn fetch_script_downloads_js_to_scripts_dir() {
        let base = spawn_remote_server(|_| String::new()).await;
        let dir = tempfile::tempdir().unwrap();
        let manager = RemoteManager::new(dir.path().to_path_buf());
        let remotes = vec![RemoteResource {
            name: "my-script".into(),
            url: format!("{base}/script.js"),
            kind: RemoteKind::Script,
            ..RemoteResource::default()
        }];

        let report = manager.fetch_all(&remotes).await;
        assert_eq!(report.fetched, 1);
        assert_eq!(report.scripts, 1);
        assert!(
            report.warnings.is_empty(),
            "warnings: {:?}",
            report.warnings
        );

        let path = dir.path().join("scripts/my-script.js");
        assert_eq!(std::fs::read_to_string(path).unwrap(), "const script = 3;");
    }

    #[tokio::test]
    async fn fetch_snippet_aggregates_and_recompiles_cached_rules() {
        let base = spawn_remote_server(|base| {
            format!(
                "[rewrite_local]\n\
                 ^https?://example\\.com/api/(.*) url-and-header https://cdn.example.com/api/$1\n\
                 ^https?://example\\.com/rsp script-response-body {base}/hook.js\n\
                 \n\
                 [task_local]\n\
                 0 9 * * * {base}/task.js, tag=每日签到\n\
                 \n\
                 [mitm]\n\
                 hostname = *.example.com, api.example2.com\n"
            )
        })
        .await;
        let dir = tempfile::tempdir().unwrap();
        let manager = RemoteManager::new(dir.path().to_path_buf());
        let remotes = vec![RemoteResource {
            name: "rules".into(),
            url: format!("{base}/snippet"),
            kind: RemoteKind::Snippet,
            dialect: ScriptDialect::QuantumultX,
            ..RemoteResource::default()
        }];

        let report = manager.fetch_all(&remotes).await;
        assert_eq!(report.fetched, 1);
        assert_eq!(report.rewrites, 1);
        assert_eq!(report.scripts, 1);
        assert_eq!(report.tasks, 1);
        assert!(
            report.warnings.is_empty(),
            "warnings: {:?}",
            report.warnings
        );

        // cache 文件已生成
        assert!(dir.path().join("remote_cache/rules.json").exists());

        // load_cached：重编译 Regex、回填 source、去重 hostname
        let merged = manager.load_cached().unwrap();
        assert_eq!(merged.rewrites.len(), 1);
        assert_eq!(
            merged.rewrites[0].pattern.as_str(),
            r"^https?://example\.com/api/(.*)"
        );
        match &merged.rewrites[0].kind {
            RewriteKind::UrlRewrite { target } => {
                assert_eq!(target, "https://cdn.example.com/api/$1");
            }
            other => panic!("unexpected kind: {other:?}"),
        }
        assert_eq!(merged.scripts.len(), 1);
        assert_eq!(merged.scripts[0].name, "hook-0");
        assert_eq!(merged.scripts[0].source, "const hook = 1;");
        assert_eq!(merged.task_scripts.len(), 1);
        assert_eq!(merged.task_scripts[0].name, "每日签到");
        assert_eq!(merged.task_scripts[0].source, "const task = 2;");
        assert_eq!(
            merged.hostnames,
            vec!["*.example.com".to_string(), "api.example2.com".to_string()]
        );
    }

    #[tokio::test]
    async fn partial_url_failure_records_warning_without_blocking_others() {
        let base = spawn_remote_server(|base| {
            format!(
                "[rewrite_local]\n\
                 ^https?://example\\.com/api/(.*) url-and-header https://cdn.example.com/api/$1\n\
                 ^https?://example\\.com/rsp script-response-body {base}/missing.js\n"
            )
        })
        .await;
        let dir = tempfile::tempdir().unwrap();
        let manager = RemoteManager::new(dir.path().to_path_buf());
        let remotes = vec![
            RemoteResource {
                name: "bad".into(),
                url: format!("{base}/snippet"),
                kind: RemoteKind::Snippet,
                dialect: ScriptDialect::QuantumultX,
                ..RemoteResource::default()
            },
            RemoteResource {
                name: "good".into(),
                url: format!("{base}/script.js"),
                kind: RemoteKind::Script,
                ..RemoteResource::default()
            },
        ];

        let report = manager.fetch_all(&remotes).await;
        // 两个 remote 均拉取成功；bad 的 hook 脚本 404 被跳过
        assert_eq!(report.fetched, 2);
        assert_eq!(report.scripts, 1); // 仅 good 脚本
        assert_eq!(report.rewrites, 1); // bad 的 rewrite 仍缓存
        assert!(
            report.warnings.iter().any(|w| w.contains("missing.js")),
            "warnings: {:?}",
            report.warnings
        );

        // good 落盘、bad snippet 缓存仍生成（rewrite 保留、scripts 跳过）
        assert!(dir.path().join("scripts/good.js").exists());
        let merged = manager.load_cached().unwrap();
        assert_eq!(merged.rewrites.len(), 1);
        assert!(merged.scripts.is_empty());
        assert!(merged.task_scripts.is_empty());
    }

    #[test]
    fn merge_imported_keeps_rewrites_hostnames_and_skips_source_empty_scripts() {
        let dir = tempfile::tempdir().unwrap();
        let manager = RemoteManager::new(dir.path().to_path_buf());
        let imported = ImportedConfig {
            rewrites: vec![RewriteRule {
                pattern: Regex::new(r"^https?://example\.com/").unwrap(),
                kind: RewriteKind::Reject,
            }],
            scripts: vec![ScriptRule {
                name: "hook-0".into(),
                kind: ScriptKind::HttpResponse,
                pattern: Regex::new(r"^https?://example\.com/rsp").unwrap(),
                requires_body: true,
                max_size: 131072,
                source: String::new(),
                argument: None,
            }],
            script_urls: vec![(
                "hook-0".to_string(),
                "https://example.com/hook.js".to_string(),
            )],
            task_scripts: vec![(
                TaskScript {
                    name: "签到".into(),
                    cron_expr: "0 0 9 * * *".into(),
                    source: String::new(),
                    dialect: ScriptDialect::QuantumultX,
                    enabled: true,
                },
                "https://example.com/task.js".to_string(),
            )],
            hostnames: vec!["*.example.com".to_string()],
            warnings: vec!["parse deviation".to_string()],
            ..Default::default()
        };

        let summary = manager.merge_imported(&imported).unwrap();
        // 重写/hostname 合入；source 为空的脚本与任务跳过计 warning
        assert_eq!(summary.rewrites, 1);
        assert_eq!(summary.hostnames, 1);
        assert_eq!(summary.scripts, 0);
        assert_eq!(summary.tasks, 0);
        assert!(summary.warnings.iter().any(|w| w.contains("hook-0")));
        assert!(summary.warnings.iter().any(|w| w.contains("签到")));
        assert!(
            summary
                .warnings
                .iter()
                .any(|w| w.contains("parse deviation"))
        );

        // 缓存已写入 remote_cache/imported.json 并被 load_cached 读取
        assert!(dir.path().join("remote_cache/imported.json").exists());
        let merged = manager.load_cached().unwrap();
        assert_eq!(merged.rewrites.len(), 1);
        assert!(merged.scripts.is_empty());
        assert!(merged.task_scripts.is_empty());
        assert_eq!(merged.hostnames, vec!["*.example.com".to_string()]);
    }

    #[test]
    fn merge_imported_appends_to_existing_import_cache() {
        let dir = tempfile::tempdir().unwrap();
        let manager = RemoteManager::new(dir.path().to_path_buf());

        let first = ImportedConfig {
            rewrites: vec![RewriteRule {
                pattern: Regex::new(r"^https?://a\.com/").unwrap(),
                kind: RewriteKind::Reject,
            }],
            scripts: vec![],
            script_urls: vec![],
            task_scripts: vec![],
            hostnames: vec!["a.example.com".to_string()],
            warnings: vec![],
            ..Default::default()
        };
        manager.merge_imported(&first).unwrap();

        let second = ImportedConfig {
            rewrites: vec![RewriteRule {
                pattern: Regex::new(r"^https?://b\.com/").unwrap(),
                kind: RewriteKind::UrlRewrite {
                    target: "https://c.com/$1".into(),
                },
            }],
            scripts: vec![],
            script_urls: vec![],
            task_scripts: vec![],
            hostnames: vec!["a.example.com".to_string(), "b.example.com".to_string()],
            warnings: vec![],
            ..Default::default()
        };
        let summary = manager.merge_imported(&second).unwrap();
        assert_eq!(summary.rewrites, 1);
        // 重复 hostname 已在缓存中，不影响本次计数（本次贡献仍为 2）
        assert_eq!(summary.hostnames, 2);

        let merged = manager.load_cached().unwrap();
        assert_eq!(merged.rewrites.len(), 2);
        assert_eq!(
            merged.hostnames,
            vec!["a.example.com".to_string(), "b.example.com".to_string()]
        );
    }

    /// 启动本地导入测试服务：提供 CamScanner 脚本与 404 端点。
    async fn spawn_import_server() -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = axum::Router::new()
            .route(
                "/camscanner.js",
                axum::routing::get(|| async { "const camscanner = 1;" }),
            )
            .route(
                "/missing.js",
                axum::routing::get(|| async { (StatusCode::NOT_FOUND, "not found") }),
            );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    #[test]
    fn detect_resource_from_url_maps_suffixes_to_kind_and_dialect() {
        assert_eq!(
            detect_resource_from_url("https://example.com/config.sgmodule"),
            Some((RemoteKind::Snippet, ScriptDialect::Surge))
        );
        assert_eq!(
            detect_resource_from_url("https://example.com/conf.plugin"),
            Some((RemoteKind::Snippet, ScriptDialect::Loon))
        );
        assert_eq!(
            detect_resource_from_url("https://example.com/rules.loon"),
            Some((RemoteKind::Snippet, ScriptDialect::Loon))
        );
        assert_eq!(
            detect_resource_from_url("https://example.com/rules.conf"),
            Some((RemoteKind::Snippet, ScriptDialect::QuantumultX))
        );
        assert_eq!(
            detect_resource_from_url("https://example.com/script.js"),
            Some((RemoteKind::Script, ScriptDialect::QuantumultX))
        );
        // 带 query / fragment：后缀判定忽略 query
        assert_eq!(
            detect_resource_from_url("https://example.com/rules.sgmodule?token=abc&x=1"),
            Some((RemoteKind::Snippet, ScriptDialect::Surge))
        );
        assert_eq!(
            detect_resource_from_url("https://example.com/script.js?token=abc#frag"),
            Some((RemoteKind::Script, ScriptDialect::QuantumultX))
        );
        // 大小写不敏感
        assert_eq!(
            detect_resource_from_url("https://example.com/RULES.SGMODULE"),
            Some((RemoteKind::Snippet, ScriptDialect::Surge))
        );
        // 无后缀 / 其他后缀 / 空串 → None
        assert_eq!(detect_resource_from_url("https://example.com/rules"), None);
        assert_eq!(
            detect_resource_from_url("https://example.com/rules.txt"),
            None
        );
        assert_eq!(detect_resource_from_url(""), None);
        // 尾斜杠仍以最后一段文件名判定后缀（目录式 URL 视作片段）
        assert_eq!(
            detect_resource_from_url("https://example.com/rules.conf/"),
            Some((RemoteKind::Snippet, ScriptDialect::QuantumultX))
        );
    }

    #[tokio::test]
    async fn import_content_fills_script_sources_and_merges_surge_sgmodule() {
        let base = spawn_import_server().await;
        let dir = tempfile::tempdir().unwrap();
        let manager = RemoteManager::new(dir.path().to_path_buf());
        let content = format!(
            "#!name=扫描全能王-解锁VIP\n\
             #!desc=扫描全能王-手机扫描仪 解锁黄金会员\n\
             #!date=2026-01-21\n\
             #!category=🐹 BOBO Premium\n\
             #!author=叮当猫chxm1023\n\
             #!icon=https://example.com/CamScanner.png\n\
             #!openUrl=https://apps.apple.com/app/id388627783\n\
             \n\
             [Script]\n\
             扫描全能王-解锁黄金会员 = type=http-response, pattern=https:\\/\\/api-cs\\.intsig\\.net\\/purchase\\/cs\\/query_property, script-path={base}/camscanner.js, requires-body=true, max-size=-1, timeout=60\n\
             \n\
             [MITM]\n\
             hostname = %APPEND% api-cs.intsig.net\n"
        );

        let summary = manager
            .import_content(&content, ScriptDialect::Surge)
            .await
            .unwrap();
        assert_eq!(summary.rewrites, 0);
        assert_eq!(summary.scripts, 1, "脚本 source 已回填，merge 不再跳过");
        assert_eq!(summary.tasks, 0);
        assert_eq!(summary.hostnames, 1);
        assert_eq!(summary.meta.name.as_deref(), Some("扫描全能王-解锁VIP"));
        assert_eq!(
            summary.meta.open_url.as_deref(),
            Some("https://apps.apple.com/app/id388627783")
        );
        assert!(
            !summary
                .warnings
                .iter()
                .any(|w| w.contains("source not fetched")),
            "不应有 source not fetched 警告: {:?}",
            summary.warnings
        );

        let merged = manager.load_cached().unwrap();
        assert_eq!(merged.scripts.len(), 1);
        assert_eq!(merged.scripts[0].source, "const camscanner = 1;");
        assert_eq!(merged.scripts[0].name, "扫描全能王-解锁黄金会员");
        assert_eq!(merged.hostnames, vec!["api-cs.intsig.net".to_string()]);
    }

    #[tokio::test]
    async fn import_content_records_warning_on_script_fetch_failure_and_keeps_rewrites() {
        let base = spawn_import_server().await;
        let dir = tempfile::tempdir().unwrap();
        let manager = RemoteManager::new(dir.path().to_path_buf());
        let content = format!(
            "#!name=失败导入\n\
             [rewrite_local]\n\
             ^https?://api-cs\\.intsig\\.net/purchase script-response-body {base}/missing.js\n\
             ^https?://example\\.com/ url-and-header https://target.example.com/\n\
             [mitm]\n\
             hostname = *.camscanner.com\n"
        );

        let summary = manager
            .import_content(&content, ScriptDialect::QuantumultX)
            .await
            .unwrap();
        assert_eq!(summary.rewrites, 1, "rewrite 不受脚本拉取失败影响");
        assert_eq!(summary.scripts, 0, "拉取失败的脚本被跳过");
        assert_eq!(summary.tasks, 0);
        assert_eq!(summary.hostnames, 1);
        assert_eq!(summary.meta.name.as_deref(), Some("失败导入"));
        assert!(
            summary.warnings.iter().any(|w| w.contains("missing.js")),
            "应有脚本拉取失败 warning: {:?}",
            summary.warnings
        );

        let merged = manager.load_cached().unwrap();
        assert_eq!(merged.rewrites.len(), 1);
        assert!(merged.scripts.is_empty());
        assert!(merged.task_scripts.is_empty());
        assert_eq!(merged.hostnames, vec!["*.camscanner.com".to_string()]);
    }

    #[tokio::test]
    async fn import_content_handles_qx_conf_sample() {
        let base = spawn_import_server().await;
        let dir = tempfile::tempdir().unwrap();
        let manager = RemoteManager::new(dir.path().to_path_buf());
        // QX 常见的 4 段式：`pattern url script-response-body <path>`（多余 `url` 修饰符忽略）
        let content = format!(
            "#!name=扫描全能王-QX\n\
             #!desc=QX 重写样例\n\
             [rewrite_local]\n\
             ^https:\\/\\/.*\\.(intsig\\.net|camscanner\\.com) url script-response-body {base}/camscanner.js\n\
             [mitm]\n\
             hostname = *.camscanner.com, *.intsig.net\n"
        );

        let summary = manager
            .import_content(&content, ScriptDialect::QuantumultX)
            .await
            .unwrap();
        assert_eq!(summary.rewrites, 0);
        assert_eq!(summary.scripts, 1, "脚本 source 已回填，merge 不再跳过");
        assert_eq!(summary.tasks, 0);
        assert_eq!(summary.hostnames, 2);
        assert_eq!(summary.meta.name.as_deref(), Some("扫描全能王-QX"));
        assert!(
            summary.warnings.is_empty(),
            "warnings: {:?}",
            summary.warnings
        );

        let merged = manager.load_cached().unwrap();
        assert_eq!(merged.scripts.len(), 1);
        assert_eq!(merged.scripts[0].name, "hook-0");
        assert_eq!(merged.scripts[0].source, "const camscanner = 1;");
        assert_eq!(
            merged.hostnames,
            vec!["*.camscanner.com".to_string(), "*.intsig.net".to_string()]
        );
    }

    /// ④ `resolve_argument_template`：用户值优先 → 默认值 → 保留原样。
    #[test]
    fn resolve_argument_template_substitutes_user_values_then_defaults() {
        let user_values = HashMap::from([("token".to_string(), "abc".to_string())]);
        let defaults = HashMap::from([("server".to_string(), "api.example.com".to_string())]);
        assert_eq!(
            resolve_argument_template("{server}|{token}", &user_values, &defaults),
            "api.example.com|abc"
        );
        // 未声明的占位保留原样。
        assert_eq!(
            resolve_argument_template("{server}|{missing}", &user_values, &defaults),
            "api.example.com|{missing}"
        );
        // 无用户值、无默认值时原样保留。
        assert_eq!(
            resolve_argument_template("{server}", &HashMap::new(), &HashMap::new()),
            "{server}"
        );
    }

    /// `apply_argument_templates`：用户值优先于默认值，未声明占位保留。
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
        let metas = vec![ConfigMeta {
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
            ..ConfigMeta::default()
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
        // argument 为 None 的规则原样保留。
        assert_eq!(out[1].argument, None);
    }

    /// `resolve_argument_template` 支持 Surge 标准三花括号占位 `{{{key}}}`（优先匹配
    /// 长形式），同时兼容简写 `{key}`；用户值优先于默认值，未声明占位保留原样。
    #[test]
    fn resolve_argument_template_supports_triple_brace_placeholders() {
        let user_values = HashMap::from([("per_filter_video".to_string(), "1".to_string())]);
        let defaults = HashMap::from([("per_filter_video".to_string(), "0".to_string())]);

        // 三花括号占位：无用户值 → 默认值 0。
        assert_eq!(
            resolve_argument_template(
                "per_filter_video_thread={{{per_filter_video}}}",
                &HashMap::new(),
                &defaults,
            ),
            "per_filter_video_thread=0"
        );
        // 三花括号占位：用户值覆盖默认值。
        assert_eq!(
            resolve_argument_template(
                "per_filter_video_thread={{{per_filter_video}}}",
                &user_values,
                &defaults,
            ),
            "per_filter_video_thread=1"
        );
        // 长形式优先：`{{{a}}}` 整体替换，不被简写 `{a}` 部分污染。
        assert_eq!(
            resolve_argument_template(
                "{{{a}}}|{a}",
                &HashMap::new(),
                &HashMap::from([("a".to_string(), "X".to_string())]),
            ),
            "X|X"
        );
        // 未声明的三花括号占位保留原样。
        assert_eq!(
            resolve_argument_template("{{{missing}}}", &HashMap::new(), &HashMap::new(),),
            "{{{missing}}}"
        );
        // 简写与三花括号共存。
        assert_eq!(
            resolve_argument_template(
                "{server}|{{{token}}}",
                &HashMap::from([("token".to_string(), "abc".to_string())]),
                &HashMap::from([("server".to_string(), "api.example.com".to_string())]),
            ),
            "api.example.com|abc"
        );
    }

    /// ⑤ `resolve_argument_template` 支持任意 key（含 `[0]` 下标）与 Loon `[{key},...]`
    /// 模板：整个 argument 串按 `{key}` 替换、外围 `[]` 保留；Surge 三花括号同样替换。
    #[test]
    fn resolve_argument_template_supports_arbitrary_keys_and_bracket_forms() {
        let user_values = HashMap::from([
            ("Types".to_string(), "Translate,External".to_string()),
            ("Languages[0]".to_string(), "AUTO".to_string()),
        ]);
        let defaults = HashMap::new();

        // Loon `[{key}]` 形式：`{Types}` / `{Languages[0]}` 逐个替换，外围方括号保留；
        // 未声明的 `{Vendor}` 保留原样。
        assert_eq!(
            resolve_argument_template("[{Types},{Languages[0]},{Vendor}]", &user_values, &defaults,),
            "[Translate,External,AUTO,{Vendor}]"
        );

        // Surge 三花括号占位含数组下标 key：整段替换、内层引号保留。
        assert_eq!(
            resolve_argument_template(
                "Types=\"{{{Types}}}\"&Languages[0]=\"{{{Languages[0]}}}\"&Vendor=\"{{{Vendor}}}\"",
                &user_values,
                &defaults,
            ),
            "Types=\"Translate,External\"&Languages[0]=\"AUTO\"&Vendor=\"{{{Vendor}}}\""
        );

        // 默认值兜底：无用户值时用 `#!arguments` 默认值。
        let with_defaults = HashMap::from([
            ("Types".to_string(), "External".to_string()),
            ("Languages[0]".to_string(), "ZH".to_string()),
        ]);
        assert_eq!(
            resolve_argument_template("[{Types},{Languages[0]}]", &HashMap::new(), &with_defaults),
            "[External,ZH]"
        );
    }

    /// ⑤ DualSubs.Spotify 全链路断言：Loon `.plugin` 的 `[Argument]` + `[Script]`
    /// 经 parse_import → apply_argument_templates 后，`[{Types},{Languages[0]},{Vendor}]`
    /// 中已声明 key 被替换、未声明的 `{Vendor}` 保留。
    #[test]
    fn dualsubs_spotify_loon_plugin_argument_template_resolution() {
        let content = r#"#!name = 🍿️ DualSubs: 🎵 Spotify
[Argument]
Types = input,"Translate,External",tag=[歌词] 启用类型（多选）,desc=请选择要添加的歌词选项。
Languages[0] = select,"AUTO","ZH","ZH-HANS","EN",tag=[翻译器] 主语言,desc=仅当源语言识别不准确时更改。
[Script]
http-response ^https?:\/\/api\.spotify\.com\/v1\/tracks\? requires-body=1, script-path=https://example.com/r.js, tag=🍿️ DualSubs.Spotify.Tracks, argument=[{Types},{Languages[0]},{Vendor}]
"#;
        let imported = parse_import(content, ScriptDialect::Loon).unwrap();

        // 由 [Argument] 构造 defaults（Types / Languages[0]）。
        let metas = [imported.meta.clone()];
        let mut defaults = HashMap::new();
        for arg in &metas[0].arguments {
            defaults.insert(arg.key.clone(), arg.default_value.clone());
        }

        // 无用户值：Types 与 Languages[0] 用默认值替换，{Vendor} 保留。
        let resolved = resolve_argument_template(
            imported.scripts[0].argument.as_deref().unwrap(),
            &HashMap::new(),
            &defaults,
        );
        assert_eq!(resolved, "[Translate,External,AUTO,{Vendor}]");

        // 用户配置值覆盖默认值。
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

    /// ⑤ Surge `.sgmodule` 的 `{{{key}}}` 占位替换（含 `Languages[0]` 下标 key）。
    #[test]
    fn dualsubs_spotify_surge_sgmodule_argument_template_resolution() {
        let content = r#"#!name = 🍿️ DualSubs: 🎵 Spotify
#!arguments = Types:"Translate,External",Languages[0]:"AUTO",Vendor:"Google"
[Script]
🍿️ DualSubs.Spotify.Tracks = type=http-response, pattern=^https?:\/\/api\.spotify\.com\/v1\/tracks\?, requires-body=1, engine=webview, script-path=https://example.com/r.js, argument=Types="{{{Types}}}"&Languages[0]="{{{Languages[0]}}}"&Vendor="{{{Vendor}}}"
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

    /// `RemoteResource` 新增字段（argument_values / icon）经 save → load 往返保留；
    /// 旧清单缺省这些字段（serde default）。
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

        // 旧清单（无新字段）读取回退为默认值，不报错。
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

    /// ⑤ prefill_argument_values：按参数声明默认值预填，已存在的用户值不覆盖；
    /// save → load 往返保留 `arguments` / `argument_values`。
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
        // server 由默认值预填；token 已有用户值不覆盖。
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

        // 旧清单（无 arguments 字段）反序列化回退为空列表。
        std::fs::write(
            manager.remotes_file(),
            r#"[{"name":"old","url":"http://example.com/x.js","kind":"Script","dialect":"Surge","update_interval_secs":86400,"enabled":true}]"#,
        )
        .unwrap();
        let legacy = manager.load().unwrap();
        assert!(legacy[0].arguments.is_empty());
    }

    /// ⑤ fetch 回填（辅助路径）：资源未声明参数而远端 meta 有声明时，fetch 后
    /// 清单回填 `arguments` 并按默认值预填 `argument_values`。
    #[tokio::test]
    async fn fetch_backfills_arguments_and_defaults_when_resource_declares_none() {
        let base = spawn_remote_server(|base| {
            format!(
                "#!name=参数模块\n\
                 #!arguments= server:api.example.com, token:default-token\n\
                 \n\
                 [Script]\n\
                 xxx = type=http-response, pattern=^https://api\\.example\\.com/, script-path={base}/hook.js, argument={{server}}|{{token}}\n"
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

    /// `#!arguments` 声明与 `argument=` 模板经 fetch → cache → load 往返不丢失，
    /// meta 透出到 `MergedRemoteConfig.metas`。
    #[tokio::test]
    async fn snippet_arguments_and_meta_roundtrip_through_cache() {
        let base = spawn_remote_server(|base| {
            format!(
                "#!name=参数模块\n\
                 #!icon=https://example.com/icon.png\n\
                 #!arguments= server:api.example.com, token:default-token\n\
                 #!arguments-desc= {{server:\"API 服务器\", token:\"鉴权令牌\"}}\n\
                 \n\
                 [Script]\n\
                 xxx = type=http-response, pattern=^https://api\\.example\\.com/, script-path={base}/hook.js, argument={{server}}|{{token}}\n"
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
        assert_eq!(meta.name.as_deref(), Some("参数模块"));
        assert_eq!(meta.icon.as_deref(), Some("https://example.com/icon.png"));
        assert_eq!(meta.arguments.len(), 2);
        let server = meta.arguments.iter().find(|a| a.key == "server").unwrap();
        assert_eq!(server.default_value, "api.example.com");
        assert_eq!(server.description.as_deref(), Some("API 服务器"));
    }

    /// ①③④ BaiDuTieBa 真实样例经 fetch → cache → load → apply 全链路：朴素 desc、
    /// 数字布尔 requires-body、无限制 max-size、三花括号占位替换为默认值/用户值。
    #[tokio::test]
    async fn snippet_badubatieba_sample_roundtrip_and_argument_resolution() {
        // `argument=` 模板原文含三花括号，用字面量避免 format 转义。
        let arg_tpl = "per_filter_video_thread={{{per_filter_video}}}";
        let base = spawn_remote_server(move |base| {
            format!(
                "#!arguments=per_filter_video:0\n\
                 #!arguments-desc=per_filter_video:设置为1则推荐页不展示视频贴\n\
                 \n\
                 [Script]\n\
                 贴吧proto = type=http-response,pattern=^https?:\\/\\/(tiebac|c\\.tieba)\\.baidu\\.com\\/...$ ,requires-body=1,binary-body-mode=1,max-size=-1,script-path={base}/hook.js,argument={arg_tpl}\n"
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

        // meta：朴素 desc 与默认值合并进 ArgSpec。
        let merged = manager.load_cached().unwrap();
        assert_eq!(merged.metas.len(), 1);
        let spec = &merged.metas[0].arguments[0];
        assert_eq!(spec.key, "per_filter_video");
        assert_eq!(spec.default_value, "0");
        assert_eq!(
            spec.description.as_deref(),
            Some("设置为1则推荐页不展示视频贴")
        );

        // 脚本钩子：requires-body=1 / max-size=-1 / 三花括号占位原文。
        assert_eq!(merged.scripts.len(), 1);
        assert_eq!(merged.scripts[0].name, "贴吧proto");
        assert!(merged.scripts[0].requires_body);
        assert_eq!(merged.scripts[0].max_size, 10 * 1024 * 1024);
        assert_eq!(
            merged.scripts[0].argument.as_deref(),
            Some("per_filter_video_thread={{{per_filter_video}}}")
        );

        // apply_argument_templates：无用户值 → 默认值 0。
        let resolved = apply_argument_templates(merged.scripts, &merged.metas, &remotes);
        assert_eq!(
            resolved[0].argument.as_deref(),
            Some("per_filter_video_thread=0")
        );

        // 用户配置值 1 → 覆盖默认值。
        let remotes_with_value = vec![RemoteResource {
            name: "baidu".into(),
            url: format!("{base}/snippet"),
            kind: RemoteKind::Snippet,
            dialect: ScriptDialect::Surge,
            argument_values: vec![("per_filter_video".to_string(), "1".to_string())],
            ..RemoteResource::default()
        }];
        let merged2 = manager.load_cached().unwrap();
        let resolved2 =
            apply_argument_templates(merged2.scripts, &merged2.metas, &remotes_with_value);
        assert_eq!(
            resolved2[0].argument.as_deref(),
            Some("per_filter_video_thread=1")
        );
    }

    /// ④ update_resource：按 name 全量更新字段、保留旧缓存；不存在的 name 报错。
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

        // 更新前写入旧缓存文件，断言更新后仍保留。
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
        assert_eq!(r.name, "rules", "name 是定位键，更新后不变");
        assert_eq!(r.url, "http://b.example.com/r2.conf");
        assert_eq!(r.dialect, ScriptDialect::Surge);
        assert_eq!(r.update_interval_secs, 3600);
        assert!(!r.enabled);
        assert_eq!(r.description.as_deref(), Some("desc"));
        assert_eq!(r.argument_values, vec![("k".to_string(), "v".to_string())]);
        assert_eq!(r.icon.as_deref(), Some("http://i.example.com/x.png"));
        assert_eq!(r.arguments.len(), 1);
        assert_eq!(r.arguments[0].key, "k");
        // 旧缓存文件保留。
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

    /// 启动本地图标测试服务：
    /// - `/icon.png`：PNG 签名字节（URL 后缀白名单路径）
    /// - `/icon.svg`：SVG 文本（换源清理旧扩展名）
    /// - `/noext`：PNG 字节、URL 无后缀（验证按字节推断扩展名）
    /// - `/empty`：空响应（验证返回 `None` 不落盘）
    async fn spawn_icon_server() -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let png: &'static [u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let app = axum::Router::new()
            .route("/icon.png", axum::routing::get(move || async move { png }))
            .route(
                "/icon.svg",
                axum::routing::get(|| async { "<svg xmlns='http://www.w3.org/2000/svg'/>" }),
            )
            .route("/noext", axum::routing::get(move || async move { png }))
            .route("/empty", axum::routing::get(|| async { "" }));
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn cache_icon_downloads_infers_extension_and_cleans_old_files() {
        let base = spawn_icon_server().await;
        let dir = tempfile::tempdir().unwrap();
        let manager = RemoteManager::new(dir.path().to_path_buf());

        // 未缓存时 icon_file 返回 None。
        assert_eq!(manager.icon_file("rules"), None);

        // URL 后缀 .png → 落盘 icons/rules.png，icon_file 命中。
        let png_path = manager
            .cache_icon("rules", &format!("{base}/icon.png"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(png_path.file_name().unwrap().to_str().unwrap(), "rules.png");
        assert_eq!(manager.icon_file("rules").unwrap(), png_path);

        // 换源为 SVG：旧 rules.png 被删除，只保留 rules.svg。
        let svg_path = manager
            .cache_icon("rules", &format!("{base}/icon.svg"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(svg_path.file_name().unwrap().to_str().unwrap(), "rules.svg");
        assert!(!dir.path().join("icons/rules.png").exists());
        assert_eq!(manager.icon_file("rules").unwrap(), svg_path);

        // URL 无后缀 → 按字节签名推断为 png。
        let sniffed = manager
            .cache_icon("weird", &format!("{base}/noext"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(sniffed.file_name().unwrap().to_str().unwrap(), "weird.png");

        // 空响应 → None，不落盘。
        assert!(
            manager
                .cache_icon("empty", &format!("{base}/empty"))
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(manager.icon_file("empty"), None);
    }
