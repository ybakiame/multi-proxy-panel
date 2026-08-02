//! MITM 代理配置与主机名匹配。

use std::net::SocketAddr;
use std::path::PathBuf;

use crate::upstream::UpstreamProxy;

/// 主机名匹配器：精确匹配或按域名后缀匹配。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostnameMatcher {
    /// 精确相等（如 `example.com`）。
    Exact(String),
    /// 匹配裸后缀本身及其所有子域（如 `*.example.com`）。
    Suffix(String),
}

impl HostnameMatcher {
    /// 判断 `host` 是否命中该匹配器。
    pub fn matches(&self, host: &str) -> bool {
        match self {
            HostnameMatcher::Exact(exact) => host == exact,
            HostnameMatcher::Suffix(suffix) => {
                host == suffix || host.ends_with(&format!(".{suffix}"))
            }
        }
    }

    /// 从通配符风格模式构造匹配器。
    ///
    /// `*.example.com` 转为 [`HostnameMatcher::Suffix`]("example.com")；
    /// 其余模式一律按精确主机名处理。
    pub fn from_pattern(pattern: &str) -> Self {
        match pattern.strip_prefix("*.") {
            Some(suffix) => HostnameMatcher::Suffix(suffix.to_string()),
            None => HostnameMatcher::Exact(pattern.to_string()),
        }
    }
}

/// MITM 代理配置。
#[derive(Debug, Clone)]
pub struct MitmConfig {
    /// 监听地址。`127.0.0.1:0` 表示随机空闲端口。
    pub listen_addr: SocketAddr,
    /// 存放 MITM CA（`ca.crt` / `ca.key`）的目录。
    pub ca_dir: PathBuf,
    /// 需要代理拦截的主机名列表。
    pub hostnames: Vec<HostnameMatcher>,
    /// 主机名排除列表：命中排除的主机不拦截（优先级高于 `hostnames` 白名单）。
    pub excluded_hostnames: Vec<HostnameMatcher>,
    /// 单个请求/响应可缓存的最大 body 字节数。
    pub max_body_size: usize,
    /// 是否启用流量记录。
    pub record_enabled: bool,
    /// 脚本钩子使用的脚本方言。
    pub script_dialect: pp_script::ScriptDialect,
    /// 上游去向：直连或经父代理（HTTP CONNECT / SOCKS5）转发。
    pub upstream: UpstreamProxy,
}

impl Default for MitmConfig {
    fn default() -> Self {
        Self {
            listen_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
            ca_dir: PathBuf::new(),
            hostnames: Vec::new(),
            excluded_hostnames: Vec::new(),
            max_body_size: 131_072,
            record_enabled: true,
            script_dialect: pp_script::ScriptDialect::Surge,
            upstream: UpstreamProxy::Direct,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_matcher_matches_only_identical_host() {
        let m = HostnameMatcher::Exact("example.com".to_string());
        assert!(m.matches("example.com"));
        assert!(!m.matches("www.example.com"));
        assert!(!m.matches("example.com.evil.com"));
        assert!(!m.matches("notexample.com"));
        assert!(!m.matches("Example.com"));
    }

    #[test]
    fn suffix_matcher_matches_host_and_subdomains() {
        let m = HostnameMatcher::Suffix("example.com".to_string());
        assert!(m.matches("example.com"));
        assert!(m.matches("www.example.com"));
        assert!(m.matches("a.b.example.com"));
        assert!(!m.matches("notexample.com"));
        assert!(!m.matches("example.com.evil.com"));
    }

    #[test]
    fn from_pattern_maps_wildcard_and_exact() {
        let wildcard = HostnameMatcher::from_pattern("*.example.com");
        assert_eq!(wildcard, HostnameMatcher::Suffix("example.com".to_string()));
        assert!(wildcard.matches("www.example.com"));
        assert!(wildcard.matches("example.com"));
        assert!(!wildcard.matches("example.org"));

        assert_eq!(
            HostnameMatcher::from_pattern("example.com"),
            HostnameMatcher::Exact("example.com".to_string())
        );
    }

    #[test]
    fn default_config_uses_sane_defaults() {
        let cfg = MitmConfig::default();
        assert_eq!(cfg.listen_addr.port(), 0);
        assert!(cfg.hostnames.is_empty());
        assert!(cfg.excluded_hostnames.is_empty());
        assert_eq!(cfg.max_body_size, 131_072);
        assert!(cfg.record_enabled);
        assert!(matches!(
            cfg.script_dialect,
            pp_script::ScriptDialect::Surge
        ));
    }
}
