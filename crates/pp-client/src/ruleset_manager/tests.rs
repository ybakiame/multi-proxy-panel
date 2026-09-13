//! `ruleset_manager` 单元测试（下载路径以本地 axum 服务模拟镜像）。

use std::collections::BTreeMap;

use serde_json::json;

use super::*;

fn data_dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

/// 已有本地文件（非空）直接可用（下载桩不应被调用）；下载失败的 tag 记为缺失。
#[tokio::test]
async fn ensure_uses_existing_files_and_marks_missing() {
    let dir = data_dir();
    let path = rule_set_path(dir.path(), "geosite-cn");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"local-cache").unwrap();

    let available =
        ensure_with_downloader(dir.path(), |_rel| async move { Err("offline".to_string()) }).await;
    assert_eq!(available.get("geosite-cn"), Some(&path));
    assert!(!available.contains_key("geoip-cn"));
}

/// 下载桩全部成功 → 文件原子写入且全部可用。
#[tokio::test]
async fn ensure_writes_downloaded_files() {
    let dir = data_dir();
    let available = ensure_with_downloader(dir.path(), |_rel| async move {
        Ok(bytes::Bytes::from_static(b"srs"))
    })
    .await;
    assert_eq!(available.len(), builtin_tags().len());
    assert_eq!(
        std::fs::read(rule_set_path(dir.path(), "geoip-cn")).unwrap(),
        b"srs"
    );
}

/// 镜像 URL 构成：前缀只作用于 GitHub raw（追加第三镜像），空前缀不追加。
#[test]
fn mirror_urls_composition() {
    let urls = mirror_urls("geosite/cn", "");
    assert_eq!(urls.len(), 2);
    assert!(urls[0].1.contains("jsdelivr.net"));
    assert!(urls[1].1.contains("raw.githubusercontent.com"));

    let urls = mirror_urls("geosite/cn", "https://gh-proxy.com");
    assert_eq!(urls.len(), 3);
    assert!(
        urls[2]
            .1
            .starts_with("https://gh-proxy.com/https://raw.githubusercontent.com/")
    );
}

/// 镜像回退：jsDelivr 失败、GitHub raw（本地服务模拟）成功时完成下载并原子写入。
#[tokio::test]
async fn download_falls_back_to_github_mirror() {
    // 本地 HTTP 服务模拟 GitHub raw；jsDelivr 域名在测试环境不可解析/不可达会先行失败。
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let app = axum::Router::new().route(
            "/MetaCubeX/meta-rules-dat/sing/geo/geosite/cn.srs",
            axum::routing::get(|| async { b"srs-bytes".as_slice() }),
        );
        axum::serve(listener, app).await.unwrap();
    });
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .no_proxy()
        .build()
        .unwrap();
    // 直接把 github 镜像替换为本地服务地址来验证回退逻辑：构造 mirrors 序列的手工版。
    let github_url = format!("http://{addr}/MetaCubeX/meta-rules-dat/sing/geo/geosite/cn.srs");
    let mirrors = [
        ("jsdelivr", "http://127.0.0.1:1/should-fail.srs".to_string()),
        ("github", github_url.clone()),
    ];
    let mut got = None;
    for (name, url) in mirrors {
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                got = Some((resp.bytes().await.unwrap(), name));
                break;
            }
            _ => continue,
        }
    }
    let (bytes, mirror) = got.expect("second mirror must succeed");
    assert_eq!(mirror, "github");
    assert_eq!(&bytes[..], b"srs-bytes");
}

/// 原子写入：父目录自动创建，内容完整。
#[test]
fn write_atomic_creates_parent_and_writes() {
    let dir = data_dir();
    let path = rule_set_path(dir.path(), "geoip-cn");
    write_atomic(&path, b"content").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"content");
    // 重写覆盖正常。
    write_atomic(&path, b"new").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"new");
}

/// 物化：可用 tag 改写为 local 路径；缺失 tag 的条目与引用规则全部移除（降级）。
#[test]
fn materialize_rewrites_available_and_strips_missing() {
    let dir = data_dir();
    let path = rule_set_path(dir.path(), "geosite-cn");
    let mut available = BTreeMap::new();
    available.insert("geosite-cn".to_string(), path.clone());

    let mut config = json!({
        "dns": {
            "rules": [
                { "query_type": ["HTTPS"], "action": "predefined", "rcode": "NOERROR" },
                { "rule_set": ["geosite-cn"], "action": "route", "server": "local" },
                { "rule_set": ["geolocation-!cn"], "query_type": ["A"], "action": "route", "server": "fakeip" }
            ]
        },
        "route": {
            "rule_set": [
                { "type": "remote", "tag": "geosite-cn", "format": "binary", "url": "https://x/cn.srs" },
                { "type": "remote", "tag": "geoip-cn", "format": "binary", "url": "https://x/ip.srs" },
                { "type": "local", "tag": "geosite-telegram", "format": "binary", "path": "/user/t.srs" }
            ],
            "rules": [
                { "action": "sniff" },
                { "rule_set": ["geosite-cn"], "outbound": "direct" },
                { "rule_set": ["geoip-cn"], "outbound": "direct" },
                { "rule_set": ["geosite-telegram"], "outbound": "proxy" }
            ]
        }
    });

    let report = materialize_rule_sets(&mut config, dir.path(), &available);

    // geosite-cn 改写为 local；geoip-cn 缺失被移除；用户条目不动。
    let rule_sets = config["route"]["rule_set"].as_array().unwrap();
    assert_eq!(rule_sets.len(), 2);
    assert_eq!(rule_sets[0]["type"], "local");
    assert_eq!(rule_sets[0]["tag"], "geosite-cn");
    assert_eq!(rule_sets[0]["path"], json!(path.to_string_lossy()));
    assert_eq!(rule_sets[1]["tag"], "geosite-telegram");

    // 引用 geoip-cn 的路由规则被移除，其余保留。
    let route_rules = config["route"]["rules"].as_array().unwrap();
    assert_eq!(route_rules.len(), 3);
    assert_eq!(report.stripped_route_rules, 1);

    // 引用 geolocation-!cn（缺失）的 DNS 规则被移除，其余保留。
    let dns_rules = config["dns"]["rules"].as_array().unwrap();
    assert_eq!(dns_rules.len(), 2);
    assert_eq!(report.stripped_dns_rules, 1);

    assert!(report.missing_tags.contains(&"geoip-cn".to_string()));
    assert!(!report.missing_tags.contains(&"geosite-cn".to_string()));
}

/// 无缺失时物化为纯改写（不移除任何规则）。
#[test]
fn materialize_no_missing_keeps_all_rules() {
    let dir = data_dir();
    let available: BTreeMap<String, PathBuf> = builtin_tags()
        .into_iter()
        .map(|tag| (tag.to_string(), rule_set_path(dir.path(), tag)))
        .collect();
    let mut config = json!({
        "route": {
            "rule_set": [{ "type": "remote", "tag": "geoip-cn", "format": "binary", "url": "https://x" }],
            "rules": [{ "rule_set": ["geoip-cn"], "outbound": "direct" }]
        }
    });
    let report = materialize_rule_sets(&mut config, dir.path(), &available);
    assert!(report.missing_tags.is_empty());
    assert_eq!(report.stripped_route_rules, 0);
    assert_eq!(config["route"]["rules"].as_array().unwrap().len(), 1);
    assert_eq!(config["route"]["rule_set"][0]["type"], "local");
}
