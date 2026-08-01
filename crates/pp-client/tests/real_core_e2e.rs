//! 真实 sing-box 核心全链路集成测试。
//!
//! 手动运行（需要真实 sing-box 二进制，避免 CI 依赖网络）：
//!
//! ```bash
//! cargo test -p pp-client --test real_core_e2e -- --include-ignored --nocapture
//! ```
//!
//! sing-box 二进制路径解析顺序：
//! 1. 环境变量 `PROXYPANEL_TEST_SINGBOX`
//! 2. `<workspace>/target/test-cores/sing-box`（即
//!    `CARGO_MANIFEST_DIR/../../target/test-cores/sing-box`）
//!
//! 二进制不存在时测试直接返回（跳过），不失败。
//!
//! 链路：reqwest → sing-box mixed 主入口（`main-in`@17890）→ 白名单域名
//! 命中路由规则 → `pp-mitm` http outbound → MITM 代理（记录流量）→ 回流
//! `mitm-return`@17891 → `direct` outbound → 本地 HTTP target server。
//! 非白名单域名不命中规则，走 `route.final` 直连 target，不经 MITM。

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::routing::get;
use pp_client::config::ClientConfig;
use pp_client::state::ClientState;
use pp_client::sysproxy::MockSystemProxy;
use pp_common::CoreType;
use pp_mitm::recorder::TrafficRecorder;

/// 客户端 mixed 主入口端口（与 core_config 测试一致）。
const MIXED_PORT: u16 = 17890;
/// 白名单域名后缀。
const WHITELIST_SUFFIX: &str = "*.example.com";
/// 白名单 target 域名（命中规则）。
const WHITELIST_HOST: &str = "whitelisted.example.com";
/// 非白名单 target 域名（直连）。
const PLAIN_HOST: &str = "plain.example.net";

/// 解析真实 sing-box 二进制路径；不存在时返回 `None`（测试跳过）。
fn singbox_binary() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("PROXYPANEL_TEST_SINGBOX") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }
    let default = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/test-cores/sing-box");
    default.is_file().then_some(default)
}

/// 启动本地 HTTP target server：所有路径返回 `target-ok`。
async fn spawn_target_server() -> (tokio::task::JoinHandle<()>, u16) {
    let app = axum::Router::new().fallback(get(|| async { "target-ok" }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (handle, port)
}

/// 启动本地订阅 server：`/sub/{token}?format=singbox` 返回含 freedom/direct
/// outbound + hosts DNS 的 sing-box 订阅配置。hosts 把两个测试域名都映射到
/// `127.0.0.1`（sing-box 1.12+ 的 `dns.servers` hosts 类型 + `predefined` 字段）。
async fn spawn_subscription_server() -> (tokio::task::JoinHandle<()>, String) {
    let sub_body = serde_json::json!({
        "log": { "level": "info" },
        "outbounds": [
            { "type": "direct", "tag": "direct" }
        ],
        "route": { "final": "direct" },
        "dns": {
            "servers": [
                {
                    "type": "hosts",
                    "tag": "hosts",
                    "predefined": {
                        WHITELIST_HOST: "127.0.0.1",
                        PLAIN_HOST: "127.0.0.1"
                    }
                }
            ]
        }
    });
    let app = axum::Router::new().route(
        "/sub/{token}",
        get(move || async move { axum::Json(sub_body.clone()) }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (handle, format!("http://{addr}"))
}

/// 轮询等待条件成立（默认 10s 超时）。
async fn wait_until<F, Fut>(mut f: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        if f().await {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for condition"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// 经 sing-box 代理发起 GET，返回响应文本；失败返回 `None`。
async fn proxied_get(client: &reqwest::Client, url: &str) -> Option<String> {
    match client.get(url).send().await {
        Ok(resp) if resp.status().is_success() => Some(resp.text().await.ok()?),
        _ => None,
    }
}

#[tokio::test]
#[ignore]
async fn real_singbox_full_chain_mitm_records_whitelisted_traffic() {
    let Some(binary) = singbox_binary() else {
        eprintln!("skipping: real sing-box binary not found; set PROXYPANEL_TEST_SINGBOX");
        return;
    };

    // 1) 本地 HTTP target + 本地订阅 server。
    let (_target_task, target_port) = spawn_target_server().await;
    let (_sub_task, hub_url) = spawn_subscription_server().await;

    // 2) ClientState：MITM 启用、白名单 `*.example.com`、不启用系统代理。
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = ClientConfig::new(
        dir.path().to_path_buf(),
        hub_url,
        "tok",
        CoreType::SingBox,
        binary,
    );
    cfg.mitm_enabled = true;
    cfg.mitm.hostnames = vec![WHITELIST_SUFFIX.to_string()];
    cfg.system_proxy_enabled = false;

    let mock = Arc::new(MockSystemProxy::new());
    let mut state = ClientState::with_system_proxy(cfg, mock.clone());

    // 3) 启动：订阅 → MITM → 合成配置 → 真实 sing-box 核心。
    state.start().await.unwrap();
    let status = state.status().await;
    assert!(status.core_running, "真实 sing-box 核心应运行");
    assert!(status.mitm_addr.is_some(), "MITM 应运行");

    // 4) reqwest 走 127.0.0.1:17890 代理。
    let client = reqwest::Client::builder()
        .proxy(
            reqwest::Proxy::http(format!("http://127.0.0.1:{MIXED_PORT}"))
                .expect("valid proxy url"),
        )
        .build()
        .unwrap();

    // 断言 1：非白名单 target → 200（核心直连路径，不经 MITM）。
    let plain_url = format!("http://{PLAIN_HOST}:{target_port}/plain");
    let client_for_wait = client.clone();
    let plain_url_for_wait = plain_url.clone();
    wait_until(move || {
        let client = client_for_wait.clone();
        let url = plain_url_for_wait.clone();
        async move { proxied_get(&client, &url).await == Some("target-ok".to_string()) }
    })
    .await;
    assert_eq!(
        proxied_get(&client, &plain_url).await.as_deref(),
        Some("target-ok"),
        "非白名单域名应直连 target"
    );

    // 断言 2：白名单 target → 200 且 recorder 有记录（流量经 MITM）。
    let wl_url = format!("http://{WHITELIST_HOST}:{target_port}/wl");
    assert_eq!(
        proxied_get(&client, &wl_url).await.as_deref(),
        Some("target-ok"),
        "白名单域名应经 MITM 后到达 target"
    );
    let records = state.recorder().list();
    assert!(
        records.iter().any(|r| r.url.contains(WHITELIST_HOST)),
        "recorder 应记录白名单请求，实际：{:?}",
        records.iter().map(|r| r.url.clone()).collect::<Vec<_>>()
    );
    // 非白名单流量不应出现在 recorder。
    assert!(
        records.iter().all(|r| !r.url.contains(PLAIN_HOST)),
        "非白名单请求不应被 MITM 记录：{:?}",
        records.iter().map(|r| r.url.clone()).collect::<Vec<_>>()
    );

    // 断言 3：stop 正常，核心与 MITM 关闭。
    state.stop().await;
    let status = state.status().await;
    assert!(!status.core_running);
    assert!(status.mitm_addr.is_none());

    _target_task.abort();
    _sub_task.abort();
}
