//! MITM 代理构建。
//!
//! 将客户端配置转换为可启动的 [`MitmProxy`]：
//! - CA 从 `client_config.mitm.ca_dir` 加载，缺失时自动生成
//! - 上游指向本机核心 mixed 入站端口（[`UpstreamProxy::Http`]）
//! - 重写引擎为空、流量记录由调用方传入

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use pp_common::PanelResult;
use pp_mitm::{
    CaStore, FileCaStore, HostnameMatcher, MitmConfig, MitmProxy, RewriteEngine, ScriptHookEngine,
    TrafficRecorder, UpstreamProxy,
};

use crate::config::ClientConfig;

/// 基于客户端配置构建 MITM 代理（不启动）。
///
/// `recorder` 由调用方注入并持有，供外部通过 `TrafficRecorder::list()` 取回抓包记录。
pub fn build_mitm_proxy(
    client_config: &ClientConfig,
    hooks: Option<ScriptHookEngine>,
    recorder: Arc<dyn TrafficRecorder>,
) -> PanelResult<MitmProxy> {
    let ca = FileCaStore::new(client_config.mitm.ca_dir.clone());
    let ca_material = ca.load_or_generate()?;

    let upstream = UpstreamProxy::Http {
        addr: SocketAddr::new(Ipv4Addr::LOCALHOST.into(), client_config.mixed_port),
    };

    let hostnames = client_config
        .mitm
        .hostnames
        .iter()
        .map(|h| HostnameMatcher::from_pattern(h))
        .collect();

    let config = MitmConfig {
        listen_addr: SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0),
        hostnames,
        upstream,
        ..MitmConfig::default()
    };

    let rewrite = RewriteEngine::default();

    Ok(MitmProxy::new(
        config,
        rewrite,
        hooks,
        recorder,
        ca_material,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pp_common::CoreType;
    use pp_mitm::MemoryRecorder;
    use std::path::PathBuf;

    use crate::config::ClientConfig;

    #[test]
    fn builds_proxy_with_generated_ca() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            "http://127.0.0.1:50052",
            "tok",
            CoreType::SingBox,
            PathBuf::from("/bin/sleep"),
        );
        let recorder: Arc<dyn TrafficRecorder> = Arc::new(MemoryRecorder::new(2048));
        let _proxy = build_mitm_proxy(&cfg, None, recorder).unwrap();

        // CA 已在 ca_dir 生成。
        assert!(cfg.mitm.ca_dir.join("ca.crt").exists());
        assert!(cfg.mitm.ca_dir.join("ca.key").exists());
        // 上游 `UpstreamProxy::Http { 127.0.0.1:mixed_port }` 在 build_mitm_proxy 内构造；
        // MitmProxy.config 为私有字段且无公开访问器，无法从外部断言，故此处仅验证 CA 生成。
    }
}
