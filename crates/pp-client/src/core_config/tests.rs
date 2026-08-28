    use super::*;
    use pp_common::{CoreType, PanelError};
    use serde_json::{Value, json};
    use std::path::PathBuf;

    fn sample_subscription() -> Value {
        json!({
            "inbounds": [{ "type": "vless", "tag": "hub-in", "listen_port": 443 }],
            "outbounds": [
                { "type": "vless", "tag": "n1", "server": "example.com", "server_port": 443 },
                { "type": "direct", "tag": "direct" }
            ],
            "route": { "final": "n1" },
            "dns": { "servers": ["1.1.1.1"] },
            "log": { "level": "info" }
        })
    }

    #[test]
    fn compose_singbox_replaces_inbounds_and_preserves_rest() {
        let sub = sample_subscription();
        let cfg = compose_singbox_config(&sub, 17890, None).unwrap();

        let inbounds = cfg["inbounds"].as_array().unwrap();
        assert_eq!(inbounds.len(), 1);
        assert_eq!(inbounds[0]["type"], "mixed");
        assert_eq!(inbounds[0]["tag"], "mixed-in");
        assert_eq!(inbounds[0]["listen"], "127.0.0.1");
        assert_eq!(inbounds[0]["listen_port"], 17890);

        assert_eq!(cfg["outbounds"], sub["outbounds"]);
        assert_eq!(cfg["route"], sub["route"]);
        assert_eq!(cfg["dns"], sub["dns"]);
        assert_eq!(cfg["log"], sub["log"]);
    }

    #[test]
    fn compose_singbox_rejects_non_object() {
        let err = compose_singbox_config(&json!([1, 2, 3]), 17890, None).unwrap_err();
        assert!(matches!(err, PanelError::Client(_)));
    }

    #[test]
    fn compose_singbox_with_mitm_chain_injects_inbounds_outbound_and_route() {
        let sub = json!({
            "outbounds": [
                { "type": "vless", "tag": "n1", "server": "example.com", "server_port": 443 }
            ],
            "route": { "final": "n1" },
        });
        let chain = MitmChain {
            proxy_addr: "127.0.0.1:34567".parse().unwrap(),
            return_port: 17891,
            hostnames: vec![
                "example.com".to_string(),
                "*.cdn.example.net".to_string(),
                "-excluded.example.org".to_string(),
            ],
        };
        let cfg = compose_singbox_config(&sub, 17890, Some(chain)).unwrap();

        // 双 mixed 入站：主入口 + 回流入口。
        let inbounds = cfg["inbounds"].as_array().unwrap();
        assert_eq!(inbounds.len(), 2);
        assert_eq!(inbounds[0]["type"], "mixed");
        assert_eq!(inbounds[0]["tag"], "main-in");
        assert_eq!(inbounds[0]["listen"], "127.0.0.1");
        assert_eq!(inbounds[0]["listen_port"], 17890);
        assert_eq!(inbounds[1]["tag"], "mitm-return");
        assert_eq!(inbounds[1]["listen_port"], 17891);

        // pp-mitm http outbound 追加在原有 outbound 之后。
        let outbounds = cfg["outbounds"].as_array().unwrap();
        assert_eq!(outbounds.len(), 2);
        assert_eq!(outbounds[1]["tag"], "pp-mitm");
        assert_eq!(outbounds[1]["type"], "http");
        assert_eq!(outbounds[1]["server"], "127.0.0.1");
        assert_eq!(outbounds[1]["server_port"], 34567);

        // route 规则前插：inbound 匹配主入口，精确/后缀正确分流，final 保留；
        // `-excluded.example.org` 为排除项，不进入路由规则。
        let rules = cfg["route"]["rules"].as_array().unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0]["inbound"], json!(["main-in"]));
        assert_eq!(rules[0]["domain"], json!(["example.com"]));
        assert_eq!(rules[0]["domain_suffix"], json!(["cdn.example.net"]));
        assert_eq!(rules[0]["outbound"], "pp-mitm");
        assert_eq!(cfg["route"]["final"], "n1");
    }

    #[test]
    fn compose_singbox_with_mitm_chain_creates_route_when_missing() {
        let sub = json!({
            "outbounds": [{ "type": "direct", "tag": "direct" }]
        });
        let chain = MitmChain {
            proxy_addr: "127.0.0.1:34567".parse().unwrap(),
            return_port: 17891,
            hostnames: vec!["example.com".to_string()],
        };
        let cfg = compose_singbox_config(&sub, 17890, Some(chain)).unwrap();

        let rules = cfg["route"]["rules"].as_array().unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0]["inbound"], json!(["main-in"]));
        assert_eq!(rules[0]["domain"], json!(["example.com"]));
        assert_eq!(rules[0]["outbound"], "pp-mitm");
    }

    /// sing-box 1.12+ 要求 `route.default_domain_resolver` 指向已声明 tag 的
    /// DNS server，否则真实 sing-box 拒绝配置（legacy resolver 缺失）。
    #[test]
    fn compose_singbox_injects_default_domain_resolver_when_dns_present() {
        let sub = json!({
            "outbounds": [{ "type": "direct", "tag": "direct" }],
            "dns": { "servers": [{ "type": "udp", "tag": "dns1", "server": "1.1.1.1" }] }
        });
        let cfg = compose_singbox_config(&sub, 17890, None).unwrap();

        assert_eq!(
            cfg["route"]["default_domain_resolver"],
            json!({ "server": "dns1" })
        );
        // 已有 tag 的 server 不被改写。
        assert_eq!(cfg["dns"]["servers"][0]["tag"], "dns1");
    }

    #[test]
    fn compose_singbox_generates_tag_for_tagless_dns_server() {
        let sub = json!({
            "outbounds": [{ "type": "direct", "tag": "direct" }],
            "dns": { "servers": [{ "type": "udp", "server": "1.1.1.1" }] }
        });
        let cfg = compose_singbox_config(&sub, 17890, None).unwrap();

        // 无 tag 的 server 自动补 tag，resolver 指向它。
        assert_eq!(cfg["dns"]["servers"][0]["tag"], "dns-0");
        assert_eq!(
            cfg["route"]["default_domain_resolver"],
            json!({ "server": "dns-0" })
        );
    }

    #[test]
    fn compose_singbox_keeps_existing_resolver_and_skips_when_no_dns() {
        // 订阅已显式声明 resolver → 不覆盖。
        let sub = json!({
            "outbounds": [{ "type": "direct", "tag": "direct" }],
            "dns": { "servers": [{ "type": "udp", "tag": "a", "server": "1.1.1.1" }] },
            "route": { "default_domain_resolver": { "server": "a" } }
        });
        let cfg = compose_singbox_config(&sub, 17890, None).unwrap();
        assert_eq!(
            cfg["route"]["default_domain_resolver"],
            json!({ "server": "a" })
        );

        // 无 dns → 不注入 resolver。
        let bare = json!({ "outbounds": [{ "type": "direct", "tag": "direct" }] });
        let cfg = compose_singbox_config(&bare, 17890, None).unwrap();
        assert!(cfg["route"].get("default_domain_resolver").is_none());
    }

    #[test]
    fn compose_mihomo_injects_mixed_and_removes_conflicting_ports() {
        let yaml = r#"
port: 7890
socks-port: 7891
redir-port: 7892
tproxy-port: 7893
proxies:
  - name: n1
    type: vless
    server: example.com
    port: 443
proxy-groups:
  - name: PROXY
    type: select
    proxies: [n1]
rules:
  - DOMAIN-SUFFIX,example.com,PROXY
dns:
  enable: true
"#;
        let cfg = compose_mihomo_config(yaml, 17890, None).unwrap();

        // mixed 入口注入，冲突的独立端口字段移除。
        assert_eq!(cfg["mixed-port"], 17890);
        assert!(cfg.get("port").is_none());
        assert!(cfg.get("socks-port").is_none());
        assert!(cfg.get("redir-port").is_none());
        assert!(cfg.get("tproxy-port").is_none());

        // 安全默认。
        assert_eq!(cfg["allow-lan"], false);
        assert_eq!(cfg["bind-address"], "127.0.0.1");

        // 其余字段原样保留（含 proxies 条目内部的 server port）。
        assert_eq!(cfg["proxies"][0]["server"], "example.com");
        assert_eq!(cfg["proxies"][0]["port"], 443);
        assert_eq!(cfg["proxy-groups"][0]["name"], "PROXY");
        assert_eq!(cfg["rules"][0], "DOMAIN-SUFFIX,example.com,PROXY");
        assert_eq!(cfg["dns"]["enable"], true);
    }

    #[test]
    fn compose_mihomo_overrides_existing_mixed_port() {
        let yaml = "mixed-port: 7890\nallow-lan: true\nproxies:\n  - name: n1\n    type: direct\n";
        let cfg = compose_mihomo_config(yaml, 17890, None).unwrap();

        assert_eq!(cfg["mixed-port"], 17890);
        assert_eq!(cfg["allow-lan"], false);
        assert_eq!(cfg["bind-address"], "127.0.0.1");
    }

    #[test]
    fn compose_mihomo_with_mitm_chain_injects_listeners_proxy_and_rules() {
        let yaml = r#"
port: 7890
proxies:
  - name: n1
    type: vless
    server: example.com
    port: 443
rules:
  - MATCH,DIRECT
"#;
        let chain = MitmChain {
            proxy_addr: "127.0.0.1:34567".parse().unwrap(),
            return_port: 17891,
            hostnames: vec![
                "example.com".to_string(),
                "*.cdn.example.net".to_string(),
                "-excluded.example.org".to_string(),
            ],
        };
        let cfg = compose_mihomo_config(yaml, 17890, Some(chain)).unwrap();

        // 主入口 + 回流入口走显式 listeners（顶层 mixed-port 被替代）。
        assert!(cfg.get("mixed-port").is_none());
        let listeners = cfg["listeners"].as_array().unwrap();
        assert_eq!(listeners.len(), 2);
        assert_eq!(listeners[0]["name"], "main-in");
        assert_eq!(listeners[0]["type"], "mixed");
        assert_eq!(listeners[0]["port"], 17890);
        assert_eq!(listeners[0]["listen"], "127.0.0.1");
        assert_eq!(listeners[1]["name"], "mitm-return");
        assert_eq!(listeners[1]["type"], "mixed");
        assert_eq!(listeners[1]["port"], 17891);

        // pp-mitm http 代理追加在原有 proxies 之后。
        let proxies = cfg["proxies"].as_array().unwrap();
        assert_eq!(proxies.len(), 2);
        let pp_mitm = proxies.iter().find(|p| p["name"] == "pp-mitm").unwrap();
        assert_eq!(pp_mitm["type"], "http");
        assert_eq!(pp_mitm["server"], "127.0.0.1");
        assert_eq!(pp_mitm["port"], 34567);
        assert_eq!(proxies[0]["name"], "n1");

        // 白名单规则前插（精确→DOMAIN，通配→DOMAIN-SUFFIX），原规则保留；
        // `-excluded.example.org` 为排除项，不生成规则。
        // 逻辑规则语法为 `LOGIC_TYPE,((payload1),(payload2)),Proxy`：AND 与
        // 子规则之间必须有逗号，否则 mihomo 无法解析。
        let rules = cfg["rules"].as_array().unwrap();
        assert_eq!(rules.len(), 3);
        assert_eq!(
            rules[0],
            "AND,((IN-NAME,main-in),(DOMAIN,example.com)),pp-mitm"
        );
        assert_eq!(
            rules[1],
            "AND,((IN-NAME,main-in),(DOMAIN-SUFFIX,cdn.example.net)),pp-mitm"
        );
        assert_eq!(rules[2], "MATCH,DIRECT");
    }

    #[test]
    fn compose_mihomo_rejects_invalid_yaml() {
        let err = compose_mihomo_config("port: [unclosed", 17890, None).unwrap_err();
        assert!(matches!(err, PanelError::Client(_)));
    }

    #[test]
    fn compose_mihomo_rejects_non_mapping_yaml() {
        let err = compose_mihomo_config("- a\n- b", 17890, None).unwrap_err();
        assert!(matches!(err, PanelError::Client(_)));
    }

    // ---------- TUN + Clash 面板注入（设置优先级最高，强制覆盖） ----------

    fn singbox_features() -> PanelFeatures {
        PanelFeatures {
            tun_enabled: true,
            tun_stack: "mixed".to_string(),
            tun_auto_route: true,
            clash_api_enabled: true,
            clash_api_port: 9090,
            clash_api_secret: "sekret".to_string(),
            clash_api_ui: "zashboard".to_string(),
            rule_mode: "rule".to_string(),
        }
    }

    #[test]
    fn apply_singbox_panel_features_injects_tun_and_clash_api() {
        let sub = json!({
            "inbounds": [{ "type": "mixed", "tag": "mixed-in", "listen": "127.0.0.1", "listen_port": 17890 }],
            "outbounds": [{ "type": "direct", "tag": "direct" }]
        });
        let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
        apply_panel_features(&mut cfg, CoreType::SingBox, &singbox_features());

        // tun 入站追加：tag / address / mtu / auto_route / stack。
        let inbounds = cfg["inbounds"].as_array().unwrap();
        let tun = inbounds
            .iter()
            .find(|i| i["type"] == "tun")
            .expect("应注入 tun 入站");
        assert_eq!(tun["tag"], "tun-in");
        assert_eq!(tun["address"], "172.19.0.1/30");
        assert_eq!(tun["mtu"], 9000);
        assert_eq!(tun["auto_route"], true);
        assert_eq!(tun["stack"], "mixed");
        assert_eq!(inbounds.len(), 2, "mixed-in 保留 + tun-in 追加");

        // experimental.clash_api 注入（含 secret）。
        assert_eq!(
            cfg["experimental"]["clash_api"]["external_controller"],
            "127.0.0.1:9090"
        );
        assert_eq!(cfg["experimental"]["clash_api"]["secret"], "sekret");
    }

    #[test]
    fn apply_singbox_panel_features_overrides_template_tun() {
        // 模板/复写已含 tun 入站与 experimental.clash_api → 以设置值为准整体替换。
        let sub = json!({
            "inbounds": [
                { "type": "tun", "tag": "tun-in", "address": "10.0.0.1/24", "mtu": 1500, "auto_route": false, "stack": "system" },
                { "type": "mixed", "tag": "mixed-in", "listen": "127.0.0.1", "listen_port": 17890 }
            ],
            "experimental": {
                "clash_api": {
                    "external_controller": "0.0.0.0:60000",
                    "external_ui": "yacd-dir",
                    "external_ui_download_url": "https://old.example/panel.zip",
                    "secret": "old"
                }
            },
            "outbounds": [{ "type": "direct", "tag": "direct" }]
        });
        let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
        apply_panel_features(&mut cfg, CoreType::SingBox, &singbox_features());

        let inbounds = cfg["inbounds"].as_array().unwrap();
        let tun: Vec<_> = inbounds.iter().filter(|i| i["type"] == "tun").collect();
        assert_eq!(tun.len(), 1, "模板 tun 被替换，只能保留一个 tun 入站");
        assert_eq!(tun[0]["address"], "172.19.0.1/30");
        assert_eq!(tun[0]["mtu"], 9000);
        assert_eq!(tun[0]["stack"], "mixed");

        // experimental.clash_api 整体替换：模板/复写自带的 external_ui 与下载地址
        // 同样以设置值为准覆盖（external_ui=ui-zashboard + 按选择的下载 URL），其余
        // experimental 字段（如有）保留。
        assert_eq!(
            cfg["experimental"]["clash_api"]["external_controller"],
            "127.0.0.1:9090"
        );
        assert_eq!(cfg["experimental"]["clash_api"]["secret"], "sekret");
        assert_eq!(
            cfg["experimental"]["clash_api"]["external_ui"],
            "ui-zashboard"
        );
        assert_eq!(
            cfg["experimental"]["clash_api"]["external_ui_download_url"],
            "https://github.com/Zephyruso/zashboard/archive/gh-pages.zip",
            "模板旧 external_ui / 下载地址应被设置值覆盖"
        );
    }

    /// Android（libbox / VpnService 接管流量）的 tun 入站必须包含 libbox 兼容字段：
    /// type / tag / address / mtu / auto_route / stack / strict_route；不含桌面专属
    /// 字段（interface_name / fd），也不含 sing-box 1.13 起移除的入站级 `sniff`
    /// （`check -c` 会拒绝）。桌面保持原字段集。
    #[test]
    fn build_singbox_tun_inbound_matches_libbox_field_set_on_android() {
        let android_tun = build_singbox_tun_inbound(&singbox_features(), true);
        assert_eq!(android_tun["type"], "tun");
        assert_eq!(android_tun["tag"], "tun-in");
        assert_eq!(android_tun["address"], "172.19.0.1/30");
        assert_eq!(android_tun["mtu"], 9000);
        assert_eq!(android_tun["auto_route"], true);
        assert_eq!(android_tun["stack"], "mixed");
        assert_eq!(android_tun["strict_route"], true);
        // 桌面专属字段不注入（libbox 经 getTunnelName(fd) 自行解析接口名）。
        assert!(android_tun.get("interface_name").is_none());
        assert!(android_tun.get("fd").is_none());
        // sing-box 1.13+ 拒绝入站级 sniff 遗留字段。
        assert!(android_tun.get("sniff").is_none());

        let desktop_tun = build_singbox_tun_inbound(&singbox_features(), false);
        assert_eq!(desktop_tun["type"], "tun");
        assert_eq!(desktop_tun["stack"], "mixed");
        assert!(
            desktop_tun.get("strict_route").is_none(),
            "桌面 tun 入站不应含 Android 专属 strict_route: {desktop_tun}"
        );
    }

    /// Android 合成配置（tun_enabled=true 且含 libbox 字段集）必须通过真实
    /// sing-box `check -c`（与 `singbox_tun_clash_api_passes_real_singbox_check`
    /// 等价，但走 Android 字段集分支）。
    #[test]
    fn android_tun_inbound_passes_real_singbox_check() {
        let Some(bin) = sing_box_binary() else {
            return;
        };
        let sub = json!({
            "outbounds": [
                { "type": "direct", "tag": "direct" }
            ],
            "route": { "final": "direct" }
        });
        let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
        // 注入 clash_api（Android 前端同样可开启）；tun 单独处理走 Android 字段集。
        let clash_only = PanelFeatures {
            tun_enabled: false,
            ..singbox_features()
        };
        apply_panel_features(&mut cfg, CoreType::SingBox, &clash_only);
        // Android 字段集 tun 入站：strict_route。
        let tun = build_singbox_tun_inbound(&singbox_features(), true);
        cfg["inbounds"].as_array_mut().unwrap().push(tun);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, serde_json::to_string_pretty(&cfg).unwrap()).unwrap();
        let out = std::process::Command::new(&bin)
            .args(["check", "-c"])
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "sing-box check failed (android tun field set): {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    // ---------- Android 显式 DNS 注入（VpnService 全量接管后系统 resolver 不可用） ----------

    #[test]
    fn inject_android_dns_sets_explicit_dns_with_actual_selector_detour() {
        let sub = json!({
            "outbounds": [
                { "type": "selector", "tag": "proxy", "outbounds": ["n1", "direct"], "default": "n1" },
                { "type": "vless", "tag": "n1", "server": "proxy-panel.ybakiame.net", "server_port": 443 },
                { "type": "direct", "tag": "direct" }
            ],
            "route": { "final": "proxy" }
        });
        let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
        inject_android_dns(&mut cfg);

        // remote 走实际 selector tag（不硬编），local 无 detour（默认直连）。
        assert_eq!(cfg["dns"]["servers"][0]["tag"], "remote");
        assert_eq!(cfg["dns"]["servers"][0]["detour"], "proxy");
        assert_eq!(cfg["dns"]["servers"][1]["tag"], "local");
        assert!(cfg["dns"]["servers"][1].get("detour").is_none());
        assert_eq!(cfg["dns"]["rules"], json!([]));
        assert_eq!(cfg["dns"]["final"], "remote");
        assert_eq!(cfg["dns"]["strategy"], "prefer_ipv4");
        // sing-box 1.12+ 要求显式 default_domain_resolver（指向首个带 tag 的 server）。
        assert_eq!(
            cfg["route"]["default_domain_resolver"],
            json!({ "server": "remote" })
        );
        // 含 server 字段的出站注入 domain_resolver → local（代理服务器域名直连解析，
        // 避免经 remote 回环）；selector 出站无 server 字段 → 不注入。
        let outbounds = cfg["outbounds"].as_array().unwrap();
        let vless = outbounds.iter().find(|o| o["tag"] == "n1").unwrap();
        assert_eq!(vless["domain_resolver"], json!({ "server": "local" }));
        let selector = outbounds.iter().find(|o| o["tag"] == "proxy").unwrap();
        assert!(
            selector.get("domain_resolver").is_none(),
            "selector 出站不应注入 domain_resolver"
        );
    }

    /// 出站已自带 `domain_resolver`（订阅/模板显式配置）时不被覆盖。
    #[test]
    fn inject_android_dns_keeps_existing_outbound_domain_resolver() {
        let sub = json!({
            "outbounds": [
                { "type": "selector", "tag": "proxy", "outbounds": ["n1"], "default": "n1" },
                {
                    "type": "vless", "tag": "n1", "server": "example.com", "server_port": 443,
                    "domain_resolver": { "server": "custom" }
                }
            ],
            "route": { "final": "proxy" }
        });
        let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
        inject_android_dns(&mut cfg);

        let outbounds = cfg["outbounds"].as_array().unwrap();
        let vless = outbounds.iter().find(|o| o["tag"] == "n1").unwrap();
        assert_eq!(
            vless["domain_resolver"],
            json!({ "server": "custom" }),
            "订阅/模板显式 domain_resolver 不被覆盖"
        );
    }

    /// 无 `server` 字段的出站（selector/urltest/direct）不注入 domain_resolver。
    #[test]
    fn inject_android_dns_does_not_inject_domain_resolver_without_server() {
        let sub = json!({
            "outbounds": [
                { "type": "selector", "tag": "proxy", "outbounds": ["auto", "direct"], "default": "auto" },
                { "type": "urltest", "tag": "auto", "outbounds": ["direct"], "url": "https://www.gstatic.com/generate_204", "interval": "5m" },
                { "type": "direct", "tag": "direct" }
            ],
            "route": { "final": "proxy" }
        });
        let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
        inject_android_dns(&mut cfg);

        for outbound in cfg["outbounds"].as_array().unwrap() {
            assert!(
                outbound.get("domain_resolver").is_none(),
                "无 server 字段的出站不应注入 domain_resolver: {outbound}"
            );
        }
    }

    /// 无 selector 时 detour 目标回退 route.final；route.final = "direct"（空 direct
    /// 出站）→ remote detour 省略（sing-box 拒绝 detour 到空 direct 出站），DNS
    /// 仍可注入。
    #[test]
    fn inject_android_dns_falls_back_to_route_final_when_no_selector() {
        let sub = json!({
            "outbounds": [{ "type": "direct", "tag": "direct" }],
            "route": { "final": "direct" }
        });
        let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
        inject_android_dns(&mut cfg);

        assert_eq!(cfg["dns"]["servers"][0]["tag"], "remote");
        assert!(
            cfg["dns"]["servers"][0].get("detour").is_none(),
            "route.final 指向空 direct 出站时 remote 应省略 detour"
        );
        assert_eq!(
            cfg["route"]["default_domain_resolver"],
            json!({ "server": "remote" })
        );
    }

    /// 订阅模式合成配置无 direct 出站：local DNS 无 detour（省略即默认直连），
    /// 不再补建 direct 出站，outbounds 保持原样。
    #[test]
    fn inject_android_dns_leaves_outbounds_untouched_when_no_direct() {
        let sub = json!({
            "outbounds": [
                { "type": "selector", "tag": "proxy", "outbounds": ["n1"], "default": "n1" },
                { "type": "vless", "tag": "n1", "server": "example.com", "server_port": 443 }
            ],
            "route": { "final": "proxy" }
        });
        let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
        inject_android_dns(&mut cfg);

        // local DNS server 无 detour（默认直连），且不补建 direct 出站。
        assert_eq!(cfg["dns"]["servers"][1]["tag"], "local");
        assert!(cfg["dns"]["servers"][1].get("detour").is_none());
        let outbounds = cfg["outbounds"].as_array().unwrap();
        assert!(
            outbounds.iter().all(|o| o["type"] != "direct"),
            "无 direct 出站时不应补建"
        );
    }

    /// 已有自定义 tag 的 direct 出站：local 仍无 detour（不引用任何 direct 出站），
    /// outbounds 不改动。
    #[test]
    fn inject_android_dns_leaves_existing_direct_outbound_untouched() {
        let sub = json!({
            "outbounds": [
                { "type": "selector", "tag": "proxy", "outbounds": ["dns-direct"], "default": "dns-direct" },
                { "type": "direct", "tag": "dns-direct" }
            ],
            "route": { "final": "proxy" }
        });
        let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
        inject_android_dns(&mut cfg);

        // local 无 detour，不引用/不改动已有 direct 出站。
        assert_eq!(cfg["dns"]["servers"][1]["tag"], "local");
        assert!(cfg["dns"]["servers"][1].get("detour").is_none());
        let direct_count = cfg["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|o| o["type"] == "direct")
            .count();
        assert_eq!(direct_count, 1, "不改动已有 direct 出站");
    }

    /// route.final 指向带额外配置键的 direct 出站（非空 direct）时，可作为 detour
    /// 目标：无 selector 场景下 remote detour 保留为 route.final 的 tag。
    #[test]
    fn inject_android_dns_keeps_detour_for_non_empty_direct() {
        let sub = json!({
            "outbounds": [
                { "type": "direct", "tag": "direct", "override_address": "1.2.3.4" }
            ],
            "route": { "final": "direct" }
        });
        let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
        inject_android_dns(&mut cfg);

        assert_eq!(cfg["dns"]["servers"][0]["tag"], "remote");
        assert_eq!(
            cfg["dns"]["servers"][0]["detour"], "direct",
            "带额外配置键的 direct 出站是合法 detour 目标，应保留 detour"
        );
    }

    /// 既无 selector 也无 route.final：无法确定 detour → 跳过注入（不产出非法配置）。
    #[test]
    fn inject_android_dns_skips_when_no_outbound_hint() {
        let sub = json!({
            "outbounds": [{ "type": "direct", "tag": "direct" }]
        });
        let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
        assert!(cfg.get("route").is_none(), "子配置无 route 无 final");
        inject_android_dns(&mut cfg);

        assert!(cfg.get("dns").is_none());
    }

    /// Android 合成配置（tun 入站 + 显式 DNS 注入）必须通过真实 `sing-box check`。
    #[test]
    fn android_config_with_injected_dns_passes_real_singbox_check() {
        let Some(bin) = sing_box_binary() else {
            return;
        };
        let sub = json!({
            "outbounds": [
                { "type": "selector", "tag": "proxy", "outbounds": ["auto", "direct"], "default": "auto" },
                { "type": "urltest", "tag": "auto", "outbounds": ["direct"], "url": "https://www.gstatic.com/generate_204", "interval": "5m" },
                { "type": "direct", "tag": "direct" }
            ],
            "route": { "final": "proxy" }
        });
        let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
        // 模拟 Android 面板注入路径：tun 入站（Android 字段集）+ clash_api + 显式 DNS。
        apply_panel_features(&mut cfg, CoreType::SingBox, &singbox_features());
        inject_android_dns(&mut cfg);

        // 合成配置的 main selector tag 为 `proxy`（singbox_template 固定组名）。
        assert_eq!(cfg["dns"]["servers"][0]["detour"], "proxy");

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, serde_json::to_string_pretty(&cfg).unwrap()).unwrap();
        let out = std::process::Command::new(&bin)
            .args(["check", "-c"])
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "sing-box check failed (android dns injection): {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// 订阅模式下合成配置完全没有 direct 出站：local DNS 省略 detour（默认直连），
    /// 不补建 direct 出站，配置必须通过真实 `sing-box check`。
    ///
    /// 注：`detour to an empty direct outbound makes no sense` 是启动阶段错误，
    /// `sing-box check`（静态校验）无法覆盖，本地断言是主要防线。
    #[test]
    fn android_config_without_direct_outbound_passes_real_singbox_check() {
        let Some(bin) = sing_box_binary() else {
            return;
        };
        let sub = json!({
            "outbounds": [
                { "type": "selector", "tag": "proxy", "outbounds": ["auto"], "default": "auto" },
                { "type": "urltest", "tag": "auto", "outbounds": ["n1"], "url": "https://www.gstatic.com/generate_204", "interval": "5m" },
                { "type": "vless", "tag": "n1", "server": "example.com", "server_port": 443 }
            ],
            "route": { "final": "proxy" }
        });
        let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
        apply_panel_features(&mut cfg, CoreType::SingBox, &singbox_features());
        inject_android_dns(&mut cfg);

        // 无 direct 出站 → local DNS 无 detour，且 outbounds 不含补建的 direct 出站。
        assert_eq!(cfg["dns"]["servers"][1]["tag"], "local");
        assert!(cfg["dns"]["servers"][1].get("detour").is_none());
        assert!(
            !cfg["outbounds"]
                .as_array()
                .unwrap()
                .iter()
                .any(|o| o["type"] == "direct"),
            "不应补建 direct 出站"
        );

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, serde_json::to_string_pretty(&cfg).unwrap()).unwrap();
        let out = std::process::Command::new(&bin)
            .args(["check", "-c"])
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "sing-box check failed (android dns injection, no direct outbound): {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[test]
    fn apply_singbox_panel_features_disabled_leaves_config_untouched() {
        let sub = json!({
            "inbounds": [{ "type": "mixed", "tag": "mixed-in", "listen": "127.0.0.1", "listen_port": 17890 }],
            "outbounds": [{ "type": "direct", "tag": "direct" }]
        });
        let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
        let disabled = PanelFeatures {
            tun_enabled: false,
            ..singbox_features()
        };
        let disabled = PanelFeatures {
            tun_stack: String::new(),
            clash_api_enabled: false,
            ..disabled
        };
        apply_panel_features(&mut cfg, CoreType::SingBox, &disabled);

        assert!(
            !cfg["inbounds"]
                .as_array()
                .unwrap()
                .iter()
                .any(|i| i["type"] == "tun")
        );
        assert!(cfg.get("experimental").is_none());
    }

    #[test]
    fn apply_mihomo_panel_features_injects_tun_and_external_controller() {
        let yaml = "mixed-port: 17890\nproxies:\n  - name: n1\n    type: direct\nrules:\n  - MATCH,DIRECT\n";
        let mut cfg = compose_mihomo_config(yaml, 17890, None).unwrap();
        apply_panel_features(&mut cfg, CoreType::Mihomo, &singbox_features());

        // tun map 注入。
        assert_eq!(cfg["tun"]["enable"], true);
        assert_eq!(cfg["tun"]["stack"], "mixed");
        assert_eq!(cfg["tun"]["auto-route"], true);
        assert_eq!(cfg["tun"]["auto-detect-interface"], true);
        assert_eq!(cfg["tun"]["dns-hijack"], json!(["any:53"]));
        // external-controller + secret 注入。
        assert_eq!(cfg["external-controller"], "127.0.0.1:9090");
        assert_eq!(cfg["secret"], "sekret");
    }

    #[test]
    fn apply_mihomo_panel_features_overrides_and_omits_empty_secret() {
        let yaml = r#"
mixed-port: 17890
tun:
  enable: false
  stack: system
external-controller: 0.0.0.0:60000
proxies:
  - name: n1
    type: direct
rules:
  - MATCH,DIRECT
"#;
        let mut cfg = compose_mihomo_config(yaml, 17890, None).unwrap();
        let features = PanelFeatures {
            tun_stack: "gvisor".to_string(),
            tun_auto_route: false,
            clash_api_secret: String::new(), // 空 secret → 省略该键
            ..singbox_features()
        };
        apply_panel_features(&mut cfg, CoreType::Mihomo, &features);

        // 模板 tun / external-controller 被设置值整体替换。
        assert_eq!(cfg["tun"]["enable"], true);
        assert_eq!(cfg["tun"]["stack"], "gvisor");
        assert_eq!(cfg["tun"]["auto-route"], false);
        assert_eq!(cfg["external-controller"], "127.0.0.1:9090");
        // secret 空串 → 输出省略。
        assert!(cfg.get("secret").is_none());
    }

    #[test]
    fn apply_mihomo_panel_features_disabled_leaves_config_untouched() {
        let yaml = "mixed-port: 17890\nproxies:\n  - name: n1\n    type: direct\nrules:\n  - MATCH,DIRECT\n";
        let mut cfg = compose_mihomo_config(yaml, 17890, None).unwrap();
        let disabled = PanelFeatures {
            tun_enabled: false,
            tun_stack: String::new(),
            clash_api_enabled: false,
            clash_api_secret: String::new(),
            ..singbox_features()
        };
        apply_panel_features(&mut cfg, CoreType::Mihomo, &disabled);

        assert!(cfg.get("tun").is_none());
        assert!(cfg.get("external-controller").is_none());
        assert!(cfg.get("secret").is_none());
        assert!(cfg.get("external-ui").is_none());
        assert!(cfg.get("external-ui-url").is_none());
    }

    // ---------- 规则模式（mihomo 顶层 mode 注入 / sing-box 不写入） ----------

    #[test]
    fn apply_mihomo_panel_features_injects_rule_mode() {
        let yaml = "mixed-port: 17890\nproxies:\n  - name: n1\n    type: direct\nrules:\n  - MATCH,DIRECT\n";
        let mut cfg = compose_mihomo_config(yaml, 17890, None).unwrap();
        let features = PanelFeatures {
            rule_mode: "global".to_string(),
            ..singbox_features()
        };
        apply_panel_features(&mut cfg, CoreType::Mihomo, &features);

        assert_eq!(cfg["mode"], "global", "mihomo 顶层应写入持久化规则模式");
    }

    /// 非法值（含空串）归一化回退 `rule` 后才写入。
    #[test]
    fn apply_mihomo_panel_features_falls_back_to_rule_for_invalid_mode() {
        let yaml = "mixed-port: 17890\nproxies:\n  - name: n1\n    type: direct\nrules:\n  - MATCH,DIRECT\n";
        for invalid in ["", "bogus", "Rule", "direct2"] {
            let mut cfg = compose_mihomo_config(yaml, 17890, None).unwrap();
            let features = PanelFeatures {
                rule_mode: invalid.to_string(),
                ..singbox_features()
            };
            apply_panel_features(&mut cfg, CoreType::Mihomo, &features);
            assert_eq!(cfg["mode"], "rule", "非法值 {invalid:?} 应回退 rule");
        }
    }

    /// 模板/复写已含 `mode` 时以设置为准整体替换。
    #[test]
    fn apply_mihomo_panel_features_mode_overrides_template() {
        let yaml = "mode: global\nmixed-port: 17890\nproxies:\n  - name: n1\n    type: direct\nrules:\n  - MATCH,DIRECT\n";
        let mut cfg = compose_mihomo_config(yaml, 17890, None).unwrap();
        assert_eq!(cfg["mode"], "global", "模板自带 mode 应被保留到注入前");
        let features = PanelFeatures {
            rule_mode: "direct".to_string(),
            ..singbox_features()
        };
        apply_panel_features(&mut cfg, CoreType::Mihomo, &features);

        assert_eq!(cfg["mode"], "direct", "设置值应覆盖模板 mode");
    }

    /// sing-box 无组合层 mode 字段：即使设置规则模式也不写入配置（运行时经
    /// Clash API `PATCH /configs` 热切换）。
    #[test]
    fn apply_singbox_panel_features_does_not_inject_mode() {
        let sub = json!({
            "outbounds": [{ "type": "direct", "tag": "direct" }]
        });
        let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
        let features = PanelFeatures {
            rule_mode: "global".to_string(),
            ..singbox_features()
        };
        apply_panel_features(&mut cfg, CoreType::SingBox, &features);

        assert!(
            cfg.get("mode").is_none(),
            "sing-box 配置不应写入顶层 mode: {cfg}"
        );
    }

    // ---------- Clash API 规则模式热切换（PATCH /configs） ----------

    /// 本地 axum 服务验证：PATCH body 为 `{"mode": ...}`、secret 非空时带 Bearer
    /// 鉴权；非 2xx 返回 Err。
    #[tokio::test]
    async fn push_clash_mode_patches_configs_with_bearer_and_checks_status() {
        use axum::http::HeaderValue;

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let captured_body = std::sync::Arc::new(std::sync::Mutex::new(None));
        let captured_auth = std::sync::Arc::new(std::sync::Mutex::new(None));
        let body_ref = std::sync::Arc::clone(&captured_body);
        let auth_ref = std::sync::Arc::clone(&captured_auth);
        let app = axum::Router::new().route(
            "/configs",
            axum::routing::patch(
                move |req: axum::http::Request<axum::body::Body>| async move {
                    *auth_ref.lock().unwrap() = req.headers().get("authorization").cloned();
                    let bytes = axum::body::to_bytes(req.into_body(), 1024).await.unwrap();
                    *body_ref.lock().unwrap() = Some(bytes.to_vec());
                    axum::http::StatusCode::NO_CONTENT
                },
            ),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        push_clash_mode(addr.port(), "sekret", "direct")
            .await
            .unwrap();
        assert_eq!(
            captured_body.lock().unwrap().as_ref().unwrap(),
            &br#"{"mode":"direct"}"#.to_vec()
        );
        assert_eq!(
            captured_auth.lock().unwrap().as_ref(),
            Some(&HeaderValue::from_static("Bearer sekret"))
        );

        // secret 空串 → 不带鉴权头。
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr2 = listener.local_addr().unwrap();
        let captured_auth2 = std::sync::Arc::new(std::sync::Mutex::new(None));
        let auth_ref2 = std::sync::Arc::clone(&captured_auth2);
        let app2 = axum::Router::new().route(
            "/configs",
            axum::routing::patch(
                move |req: axum::http::Request<axum::body::Body>| async move {
                    *auth_ref2.lock().unwrap() = req.headers().get("authorization").cloned();
                    axum::http::StatusCode::NO_CONTENT
                },
            ),
        );
        tokio::spawn(async move {
            axum::serve(listener, app2).await.unwrap();
        });
        push_clash_mode(addr2.port(), "", "rule").await.unwrap();
        assert!(
            captured_auth2.lock().unwrap().as_ref().is_none(),
            "空 secret 不应带 Authorization 头"
        );

        // 非 2xx → Err。
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr3 = listener.local_addr().unwrap();
        let app3 = axum::Router::new().route(
            "/configs",
            axum::routing::patch(|| async { axum::http::StatusCode::BAD_REQUEST }),
        );
        tokio::spawn(async move {
            axum::serve(listener, app3).await.unwrap();
        });
        assert!(push_clash_mode(addr3.port(), "", "direct").await.is_err());
    }

    /// 重试语义：核心刚启动时 Clash API 可能未就绪（前两次 500），第三次成功 →
    /// 重试后返回 Ok，且请求次数 = 3。
    #[tokio::test]
    async fn push_clash_mode_retries_transient_failure_until_success() {
        let attempts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let attempts_ref = std::sync::Arc::clone(&attempts);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = axum::Router::new().route(
            "/configs",
            axum::routing::patch(move || async move {
                let n = attempts_ref.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if n < 2 {
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR
                } else {
                    axum::http::StatusCode::NO_CONTENT
                }
            }),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        push_clash_mode(addr.port(), "", "global").await.unwrap();
        assert_eq!(
            attempts.load(std::sync::atomic::Ordering::SeqCst),
            3,
            "前两次失败后第三次应重试成功"
        );
    }

    /// 重试语义：全部 3 次失败 → Err（调用方 best-effort 记 warning 不阻断）。
    #[tokio::test]
    async fn push_clash_mode_returns_err_when_all_retries_fail() {
        let attempts = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let attempts_ref = std::sync::Arc::clone(&attempts);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = axum::Router::new().route(
            "/configs",
            axum::routing::patch(move || async move {
                attempts_ref.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                axum::http::StatusCode::INTERNAL_SERVER_ERROR
            }),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        assert!(push_clash_mode(addr.port(), "", "global").await.is_err());
        assert_eq!(
            attempts.load(std::sync::atomic::Ordering::SeqCst),
            3,
            "全部失败时最多重试 3 次"
        );
    }

    // ---------- Clash 面板 UI 选择（yacd / zashboard / metacubexd） ----------

    fn features_with_ui(ui: &str) -> PanelFeatures {
        PanelFeatures {
            clash_api_ui: ui.to_string(),
            ..singbox_features()
        }
    }

    /// 三种 UI 选择的 sing-box 注入断言。
    #[test]
    fn apply_singbox_panel_features_injects_external_ui_download_url() {
        let cases = [
            (
                "yacd",
                "https://github.com/haishanh/yacd/archive/gh-pages.zip",
            ),
            (
                "zashboard",
                "https://github.com/Zephyruso/zashboard/archive/gh-pages.zip",
            ),
            (
                "metacubexd",
                "https://github.com/MetaCubeX/metacubexd/archive/gh-pages.zip",
            ),
        ];
        for (ui, url) in cases {
            let sub = json!({
                "outbounds": [{ "type": "direct", "tag": "direct" }]
            });
            let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
            apply_panel_features(&mut cfg, CoreType::SingBox, &features_with_ui(ui));

            assert_eq!(
                cfg["experimental"]["clash_api"]["external_ui"],
                format!("ui-{}", normalized_clash_api_ui(ui)),
                "UI 选择 {ui}"
            );
            assert_eq!(
                cfg["experimental"]["clash_api"]["external_ui_download_url"], url,
                "UI 选择 {ui}"
            );
        }
    }

    /// 三种 UI 选择的 mihomo 注入断言。
    #[test]
    fn apply_mihomo_panel_features_injects_external_ui_download_url() {
        let cases = [
            (
                "yacd",
                "https://github.com/haishanh/yacd/archive/gh-pages.zip",
            ),
            (
                "zashboard",
                "https://github.com/Zephyruso/zashboard/archive/gh-pages.zip",
            ),
            (
                "metacubexd",
                "https://github.com/MetaCubeX/metacubexd/archive/gh-pages.zip",
            ),
        ];
        for (ui, url) in cases {
            let yaml = "mixed-port: 17890\nproxies:\n  - name: n1\n    type: direct\nrules:\n  - MATCH,DIRECT\n";
            let mut cfg = compose_mihomo_config(yaml, 17890, None).unwrap();
            apply_panel_features(&mut cfg, CoreType::Mihomo, &features_with_ui(ui));

            assert_eq!(
                cfg["external-ui"],
                format!("ui-{}", normalized_clash_api_ui(ui)),
                "UI 选择 {ui}"
            );
            assert_eq!(cfg["external-ui-url"], url, "UI 选择 {ui}");
        }
    }

    /// Android（`is_android=true`）：mihomo 在 ApplyConfig 路径同步下载
    /// external-ui 面板 zip，阻塞 setup 拖慢启动；本应用自带 UI 面板无价值。
    /// Android 分支只写 external-controller + secret，不写 external-ui /
    /// external-ui-url，并移除模板/复写自带的三个面板 UI 键（含
    /// external-ui-name）。桌面行为由既有测试覆盖，不受影响。
    #[test]
    fn apply_mihomo_panel_features_android_omits_external_ui_and_keeps_controller() {
        let yaml = r#"
mixed-port: 17890
external-controller: 0.0.0.0:60000
external-ui: ui
external-ui-url: https://github.com/haishanh/yacd/archive/gh-pages.zip
external-ui-name: yacd
proxies:
  - name: n1
    type: direct
rules:
  - MATCH,DIRECT
"#;
        let mut cfg = compose_mihomo_config(yaml, 17890, None).unwrap();
        let features = PanelFeatures {
            clash_api_secret: "sekret".to_string(),
            ..singbox_features()
        };
        apply_mihomo_panel_features_impl(&mut cfg, &features, true);

        // external-controller / secret 保留（Clash API 规则模式热切换依赖）。
        assert_eq!(cfg["external-controller"], "127.0.0.1:9090");
        assert_eq!(cfg["secret"], "sekret");
        // 不写 external-ui / external-ui-url；模板/复写自带的面板 UI 键被移除。
        assert!(
            cfg.get("external-ui").is_none(),
            "Android 不应写 external-ui: {cfg}"
        );
        assert!(
            cfg.get("external-ui-url").is_none(),
            "Android 不应写 external-ui-url: {cfg}"
        );
        assert!(
            cfg.get("external-ui-name").is_none(),
            "模板自带 external-ui-name 应被移除: {cfg}"
        );
    }

    /// Android（`is_android=true`）+ 空 secret：external-controller 保留、secret
    /// 省略、模板自带面板 UI 键移除。
    #[test]
    fn apply_mihomo_panel_features_android_omits_empty_secret_and_template_ui() {
        let yaml = "mixed-port: 17890\nexternal-ui: ui\nexternal-ui-url: https://old.example/panel.zip\nproxies:\n  - name: n1\n    type: direct\nrules:\n  - MATCH,DIRECT\n";
        let mut cfg = compose_mihomo_config(yaml, 17890, None).unwrap();
        let features = PanelFeatures {
            clash_api_secret: String::new(), // 空 secret → 省略该键
            ..singbox_features()
        };
        apply_mihomo_panel_features_impl(&mut cfg, &features, true);

        assert_eq!(cfg["external-controller"], "127.0.0.1:9090");
        assert!(cfg.get("secret").is_none());
        assert!(cfg.get("external-ui").is_none());
        assert!(cfg.get("external-ui-url").is_none());
    }

    /// 未知值 / 空串回退 zashboard（映射函数 + 两核心注入路径）。
    #[test]
    fn clash_api_ui_unknown_falls_back_to_zashboard() {
        assert_eq!(
            clash_api_ui_download_url("unknown-ui"),
            "https://github.com/Zephyruso/zashboard/archive/gh-pages.zip"
        );
        assert_eq!(
            clash_api_ui_download_url(""),
            "https://github.com/Zephyruso/zashboard/archive/gh-pages.zip"
        );
        assert_eq!(
            clash_api_ui_download_url("zashboard"),
            "https://github.com/Zephyruso/zashboard/archive/gh-pages.zip"
        );

        let sub = json!({
            "outbounds": [{ "type": "direct", "tag": "direct" }]
        });
        let mut sb = compose_singbox_config(&sub, 17890, None).unwrap();
        apply_panel_features(&mut sb, CoreType::SingBox, &features_with_ui("bogus"));
        assert_eq!(
            sb["experimental"]["clash_api"]["external_ui_download_url"],
            "https://github.com/Zephyruso/zashboard/archive/gh-pages.zip"
        );
        assert_eq!(
            sb["experimental"]["clash_api"]["external_ui"], "ui-zashboard",
            "未知值回退 zashboard 时目录名同步回退"
        );

        let yaml = "mixed-port: 17890\nproxies:\n  - name: n1\n    type: direct\nrules:\n  - MATCH,DIRECT\n";
        let mut mh = compose_mihomo_config(yaml, 17890, None).unwrap();
        apply_panel_features(&mut mh, CoreType::Mihomo, &features_with_ui("bogus"));
        assert_eq!(
            mh["external-ui-url"],
            "https://github.com/Zephyruso/zashboard/archive/gh-pages.zip"
        );
        assert_eq!(
            mh["external-ui"], "ui-zashboard",
            "未知值回退 zashboard 时目录名同步回退"
        );
    }

    /// 切换选择后 external_ui 目录名不同——这是切换生效的关键：核心仅在目录不存在
    /// 时下载面板 zip，固定 `ui` 目录时切换选择只改下载 URL，目录已有旧面板永远
    /// 不会重新下载（重启后仍是旧面板）。目录按选择区分后，新选择走新目录触发
    /// 重新下载，旧目录残留不影响。两核心一致。
    #[test]
    fn switching_ui_choice_changes_external_ui_dir_for_both_cores() {
        let sub = json!({
            "outbounds": [{ "type": "direct", "tag": "direct" }]
        });
        let mut sb_yacd = compose_singbox_config(&sub, 17890, None).unwrap();
        apply_panel_features(&mut sb_yacd, CoreType::SingBox, &features_with_ui("yacd"));
        let mut sb_zash = compose_singbox_config(&sub, 17890, None).unwrap();
        apply_panel_features(
            &mut sb_zash,
            CoreType::SingBox,
            &features_with_ui("zashboard"),
        );

        assert_eq!(
            sb_yacd["experimental"]["clash_api"]["external_ui"], "ui-yacd",
            "sing-box yacd 目录"
        );
        assert_eq!(
            sb_zash["experimental"]["clash_api"]["external_ui"], "ui-zashboard",
            "sing-box zashboard 目录"
        );
        assert_ne!(
            sb_yacd["experimental"]["clash_api"]["external_ui"],
            sb_zash["experimental"]["clash_api"]["external_ui"],
            "sing-box 切换选择后 external_ui 目录必须不同"
        );

        let yaml = "mixed-port: 17890\nproxies:\n  - name: n1\n    type: direct\nrules:\n  - MATCH,DIRECT\n";
        let mut mh_yacd = compose_mihomo_config(yaml, 17890, None).unwrap();
        apply_panel_features(&mut mh_yacd, CoreType::Mihomo, &features_with_ui("yacd"));
        let mut mh_zash = compose_mihomo_config(yaml, 17890, None).unwrap();
        apply_panel_features(
            &mut mh_zash,
            CoreType::Mihomo,
            &features_with_ui("zashboard"),
        );

        assert_eq!(mh_yacd["external-ui"], "ui-yacd", "mihomo yacd 目录");
        assert_eq!(
            mh_zash["external-ui"], "ui-zashboard",
            "mihomo zashboard 目录"
        );
        assert_ne!(
            mh_yacd["external-ui"], mh_zash["external-ui"],
            "mihomo 切换选择后 external-ui 目录必须不同"
        );
    }

    // ---------- 真实核心 check（target/test-cores 存在时验证 tun + clash_api 通过） ----------

    /// 真实核心二进制目录：`target/test-cores`（工作区根下）。缺失时相关测试直接跳过。
    fn test_core_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/test-cores")
    }

    fn sing_box_binary() -> Option<PathBuf> {
        let p = test_core_dir().join("sing-box");
        p.is_file().then_some(p)
    }

    fn mihomo_binary() -> Option<PathBuf> {
        let p = test_core_dir().join("mihomo");
        p.is_file().then_some(p)
    }

    /// 本地已下载的 mihomo geoip.metadb（`~/.config/mihomo`），避免 `mihomo -t` 联网下载。
    fn geoip_metadb() -> Option<PathBuf> {
        let p = PathBuf::from(std::env::var("HOME").unwrap_or_default())
            .join(".config/mihomo/geoip.metadb");
        p.is_file().then_some(p)
    }

    #[test]
    fn singbox_tun_clash_api_passes_real_singbox_check() {
        let Some(bin) = sing_box_binary() else {
            return;
        };
        let sub = json!({
            "outbounds": [
                { "type": "direct", "tag": "direct" }
            ],
            "route": { "final": "direct" }
        });
        let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
        // 用非默认 UI（metacubexd）验证 external_ui / external_ui_download_url 注入
        // 后真实 sing-box check 仍通过。
        apply_panel_features(&mut cfg, CoreType::SingBox, &features_with_ui("metacubexd"));

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, serde_json::to_string_pretty(&cfg).unwrap()).unwrap();
        let out = std::process::Command::new(&bin)
            .args(["check", "-c"])
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "sing-box check failed (tun + clash_api): {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[test]
    fn mihomo_tun_clash_api_passes_real_mihomo_check() {
        let Some(bin) = mihomo_binary() else {
            return;
        };
        let yaml = "mixed-port: 17890\nproxies:\n  - name: n1\n    type: direct\nrules:\n  - MATCH,DIRECT\n";
        let mut cfg = compose_mihomo_config(yaml, 17890, None).unwrap();
        // 用非默认 UI（metacubexd）验证 external-ui / external-ui-url 注入后真实
        // mihomo check 仍通过。
        apply_panel_features(&mut cfg, CoreType::Mihomo, &features_with_ui("metacubexd"));

        let dir = tempfile::tempdir().unwrap();
        // 预置 geoip.metadb（存在时）避免 `mihomo -t` 联网下载 geo 数据。
        if let Some(mmdb) = geoip_metadb() {
            std::fs::copy(mmdb, dir.path().join("geoip.metadb")).unwrap();
        }
        let path = dir.path().join("config.yaml");
        std::fs::write(&path, serde_yaml::to_string(&cfg).unwrap()).unwrap();
        let out = std::process::Command::new(&bin)
            .args(["-t", "-f"])
            .arg(&path)
            .arg("-d")
            .arg(dir.path())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "mihomo check failed (tun + clash_api): {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
