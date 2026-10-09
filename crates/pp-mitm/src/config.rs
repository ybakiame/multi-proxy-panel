//! MITM 代理配置与主机名匹配。

use std::net::SocketAddr;
use std::path::PathBuf;

use crate::upstream::UpstreamProxy;

/// 主机名匹配器：精确匹配、域名后缀匹配或通配符正则匹配。
#[derive(Debug, Clone)]
pub enum HostnameMatcher {
    /// 精确相等（如 `example.com`）。
    Exact(String),
    /// 匹配裸后缀本身及其所有子域（如 `*.example.com`）。
    Suffix(String),
    /// 含任意位置 `*` 的通配模式（如 `api-*.example.com` / `*xmind.*`），
    /// 转正则后全串锚定匹配；`pattern` 保留原始文本用于比较/展示。
    Wildcard {
        pattern: String,
        regex: regex::Regex,
    },
}

impl PartialEq for HostnameMatcher {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (HostnameMatcher::Exact(a), HostnameMatcher::Exact(b)) => a == b,
            (HostnameMatcher::Suffix(a), HostnameMatcher::Suffix(b)) => a == b,
            (
                HostnameMatcher::Wildcard { pattern: a, .. },
                HostnameMatcher::Wildcard { pattern: b, .. },
            ) => a == b,
            _ => false,
        }
    }
}
impl Eq for HostnameMatcher {}

impl HostnameMatcher {
    /// 判断 `host` 是否命中该匹配器（`host` 不应携带端口，调用方先行剥离）。
    pub fn matches(&self, host: &str) -> bool {
        match self {
            HostnameMatcher::Exact(exact) => host == exact,
            HostnameMatcher::Suffix(suffix) => {
                host == suffix || host.ends_with(&format!(".{suffix}"))
            }
            HostnameMatcher::Wildcard { regex, .. } => regex.is_match(host),
        }
    }

    /// 从通配符风格模式构造匹配器。
    ///
    /// - `*.example.com` → [`HostnameMatcher::Suffix`]
    /// - 含其余位置 `*`（如 `api-*.example.com` / `*xmind.*`）→ [`HostnameMatcher::Wildcard`]
    ///   （`*` 转 `.*`，全串锚定；无法编译时按 [`HostnameMatcher::Exact`] 兜底）
    /// - 尾部 `:port` 一律剥离（生态 snippet 常见 `host:443` 写法，拦截判定只看主机名）
    /// - 其余按精确主机名处理
    pub fn from_pattern(pattern: &str) -> Self {
        let pattern = strip_port(pattern);
        if let Some(suffix) = pattern.strip_prefix("*.") {
            return HostnameMatcher::Suffix(suffix.to_string());
        }
        if pattern.contains('*') {
            let regex_src = format!("^{}$", regex::escape(pattern).replace(r"\*", ".*"));
            if let Ok(regex) = regex::Regex::new(&regex_src) {
                return HostnameMatcher::Wildcard {
                    pattern: pattern.to_string(),
                    regex,
                };
            }
        }
        HostnameMatcher::Exact(pattern.to_string())
    }
}

/// 剥离主机名尾部的 `:port`（纯数字）；IPv6 字面量不处理（生态中几乎不出现）。
fn strip_port(host: &str) -> &str {
    match host.rsplit_once(':') {
        Some((h, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => h,
        _ => host,
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
    fn mid_string_wildcard_matches_via_regex() {
        let m = HostnameMatcher::from_pattern("api-*.facereplacerext.com");
        assert!(m.matches("api-cn.facereplacerext.com"));
        assert!(m.matches("api-.facereplacerext.com"));
        assert!(!m.matches("apicn.facereplacerext.com"));
        assert!(!m.matches("api-cn.facereplacerext.com.evil.com"));

        let m = HostnameMatcher::from_pattern("*xmind.*");
        assert!(m.matches("www.xmind.com"));
        assert!(m.matches("xmind.app"));
        assert!(!m.matches("xmind2.com"));

        // 尾部通配与 IP 形态。
        let m = HostnameMatcher::from_pattern("api.meiease.*");
        assert!(m.matches("api.meiease.com"));
        assert!(m.matches("api.meiease.cn"));
        // 贪婪通配语义（与 QX/Loon 一致）：`.` 后任意串均命中。
        assert!(m.matches("api.meiease.com.evil.org"));
        assert!(!m.matches("api.meiease2.com"));
        let m = HostnameMatcher::from_pattern("39.156.12*.*");
        assert!(m.matches("39.156.123.46"));
        assert!(!m.matches("39.155.13.46"));
    }

    #[test]
    fn port_suffix_is_stripped() {
        assert_eq!(
            HostnameMatcher::from_pattern("*.skyjos.com:58080"),
            HostnameMatcher::Suffix("skyjos.com".to_string())
        );
        assert_eq!(
            HostnameMatcher::from_pattern("example.com:443"),
            HostnameMatcher::Exact("example.com".to_string())
        );
        // 非数字端口不剥离（非主机名形态，按原文精确匹配）。
        assert_eq!(
            HostnameMatcher::from_pattern("example.com:abc"),
            HostnameMatcher::Exact("example.com:abc".to_string())
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
