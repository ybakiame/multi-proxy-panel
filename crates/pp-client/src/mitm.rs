//! MITM 代理构建。
//!
//! 将客户端配置转换为可启动的 [`MitmProxy`]：
//! - CA 从 `client_config.mitm.ca_dir` 加载，缺失时自动生成
//! - 上游指向本机核心回流 mixed 入站端口（[`UpstreamProxy::Http`]，默认
//!   `mixed_port + 1`，可通过 [`MitmBuildOptions::upstream_port`] 覆盖）
//! - hostname 白名单中 `-` / `!` 前缀条目（排除项）剥前缀后放入
//!   [`MitmConfig::excluded_hostnames`]，其余放入 `hostnames`
//! - 重写引擎为空、流量记录由调用方传入

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use pp_common::PanelResult;
use pp_mitm::{
    CaStore, FileCaStore, HostnameMatcher, MitmConfig, MitmProxy, RewriteEngine, ScriptHookEngine,
    TrafficRecorder, UpstreamProxy,
};

use crate::config::ClientConfig;

/// MITM 构建的可选注入项（远程订阅：重写规则 / 主机名 / 脚本钩子）。
#[derive(Default)]
pub struct MitmBuildOptions {
    /// 额外主机名（可含 `-` / `!` 前缀排除项），与
    /// `client_config.mitm.hostnames` 合并去重。
    pub extra_hostnames: Vec<String>,
    /// MITM 上游（回流）端口；`None` 时默认 `client_config.mixed_port + 1`。
    pub upstream_port: Option<u16>,
    /// 重写规则引擎（为空时使用空引擎）。
    pub rewrite: RewriteEngine,
    /// 脚本钩子引擎。
    pub hooks: Option<ScriptHookEngine>,
}

/// 基于客户端配置构建 MITM 代理（不启动）。
///
/// 上游指向本机核心回流 mixed 入站端口（[`UpstreamProxy::Http`]，默认
/// `mixed_port + 1`），即 MITM 解密后的流量回到核心继续正常路由。
/// hostname 中 `-` / `!` 前缀条目为排除项（剥前缀后进入
/// `MitmConfig.excluded_hostnames`，命中不拦截），其余为白名单。
/// `recorder` 由调用方注入并持有，供外部通过 `TrafficRecorder::list()` 取回抓包记录；
/// `options` 提供远程订阅合并的额外 hostname / 重写规则 / 脚本钩子。
pub fn build_mitm_proxy(
    client_config: &ClientConfig,
    options: MitmBuildOptions,
    recorder: Arc<dyn TrafficRecorder>,
) -> PanelResult<MitmProxy> {
    let ca = FileCaStore::new(client_config.mitm.ca_dir.clone());
    let ca_material = ca.load_or_generate()?;

    let upstream_port = options
        .upstream_port
        .unwrap_or(client_config.mixed_port + 1);
    let upstream = UpstreamProxy::Http {
        addr: SocketAddr::new(Ipv4Addr::LOCALHOST.into(), upstream_port),
    };

    let mut hostname_sources = client_config.mitm.hostnames.clone();
    for extra in options.extra_hostnames {
        if !hostname_sources.contains(&extra) {
            hostname_sources.push(extra);
        }
    }
    // `-` / `!` 前缀条目为排除项（见 import 解析），剥前缀后构造排除匹配器；
    // 其余为白名单匹配器。
    let mut hostnames = Vec::new();
    let mut excluded_hostnames = Vec::new();
    for hostname in &hostname_sources {
        if let Some(pattern) = hostname
            .strip_prefix('-')
            .or_else(|| hostname.strip_prefix('!'))
        {
            excluded_hostnames.push(HostnameMatcher::from_pattern(pattern));
        } else {
            hostnames.push(HostnameMatcher::from_pattern(hostname));
        }
    }

    let config = MitmConfig {
        listen_addr: SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 0),
        hostnames,
        excluded_hostnames,
        upstream,
        ..MitmConfig::default()
    };

    Ok(MitmProxy::new(
        config,
        options.rewrite,
        options.hooks,
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
        let _proxy = build_mitm_proxy(&cfg, MitmBuildOptions::default(), recorder).unwrap();

        // CA 已在 ca_dir 生成。
        assert!(cfg.mitm.ca_dir.join("ca.crt").exists());
        assert!(cfg.mitm.ca_dir.join("ca.key").exists());
        // 上游 `UpstreamProxy::Http { 127.0.0.1:mixed_port+1 }` 在 build_mitm_proxy 内构造；
        // MitmProxy.config 为私有字段且无公开访问器，无法从外部断言，故此处仅验证 CA 生成。
    }
}
