//! 主机名拦截判定。
//!
//! CONNECT（按目标主机名）与 TLS ClientHello（按 SNI）两个 hudsucker 钩子
//! 共用的白名单 / 排除列表匹配逻辑。

use crate::config::MitmConfig;

/// 依据配置判断 `host` 是否应拦截。
///
/// 排除列表（`excluded_hostnames`）优先级高于白名单：命中排除的主机一律不拦截；
/// 白名单为空时除排除命中外全量拦截。
pub(crate) fn should_intercept_host(config: &MitmConfig, host: &str) -> bool {
    if config.excluded_hostnames.iter().any(|m| m.matches(host)) {
        return false;
    }
    if config.hostnames.is_empty() {
        return true;
    }
    config.hostnames.iter().any(|m| m.matches(host))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::HostnameMatcher;

    /// 构造带指定 hostnames / excluded_hostnames 的配置。
    fn test_config(
        hostnames: Vec<HostnameMatcher>,
        excluded_hostnames: Vec<HostnameMatcher>,
    ) -> MitmConfig {
        MitmConfig {
            hostnames,
            excluded_hostnames,
            ..MitmConfig::default()
        }
    }

    /// CONNECT 请求：uri 为 `host:443` 形式，返回值与
    /// `should_intercept_connect` 内部提取的 host 一致（`vip.iqiyi.com:443` → `vip.iqiyi.com`）。
    fn connect_host(host: &str) -> String {
        host.split(':').next().unwrap_or_default().to_string()
    }

    #[test]
    fn should_intercept_connect_excluded_host_wins_over_whitelist() {
        // `*.iqiyi.com` 在白名单，`vip.iqiyi.com` 同时被排除 → 排除命中时不拦截。
        let config = test_config(
            vec![HostnameMatcher::Suffix("iqiyi.com".to_string())],
            vec![HostnameMatcher::Suffix("vip.iqiyi.com".to_string())],
        );
        assert!(
            !should_intercept_host(&config, &connect_host("vip.iqiyi.com")),
            "排除命中即使白名单命中也不应拦截"
        );
    }

    #[test]
    fn should_intercept_connect_whitelist_hit_intercepts() {
        let config = test_config(
            vec![HostnameMatcher::Suffix("iqiyi.com".to_string())],
            vec![HostnameMatcher::Exact("blocked.example.com".to_string())],
        );
        assert!(
            should_intercept_host(&config, &connect_host("api.iqiyi.com")),
            "未排除且白名单命中应拦截"
        );
        assert!(
            !should_intercept_host(&config, &connect_host("blocked.example.com")),
            "白名单未命中不应拦截"
        );
    }

    #[test]
    fn should_intercept_connect_empty_whitelist_intercepts_unless_excluded() {
        let config = test_config(
            Vec::new(),
            vec![HostnameMatcher::Suffix("vip.iqiyi.com".to_string())],
        );
        assert!(
            should_intercept_host(&config, &connect_host("example.com")),
            "白名单为空时未命中排除应全量拦截"
        );
        assert!(
            !should_intercept_host(&config, &connect_host("vip.iqiyi.com")),
            "白名单为空时命中排除仍不拦截"
        );
    }

    // should_intercept_tls 与 should_intercept_connect 共用 should_intercept_host 判定
    // （ClientHello 无法在测试中构造，SNI 经 `server_name().unwrap_or_default()` 提取后
    // 走同一函数），以下用例按 SNI 语义复核白名单命中 / 未命中 / 排除命中。

    #[test]
    fn should_intercept_tls_whitelist_hit_intercepts() {
        let config = test_config(
            vec![HostnameMatcher::Suffix("iqiyi.com".to_string())],
            vec![HostnameMatcher::Exact("blocked.example.com".to_string())],
        );
        assert!(
            should_intercept_host(&config, "api.iqiyi.com"),
            "SNI 未排除且白名单命中应拦截"
        );
        assert!(
            !should_intercept_host(&config, "other.example.com"),
            "SNI 白名单未命中不应拦截（走盲隧道透传）"
        );
    }

    #[test]
    fn should_intercept_tls_excluded_sni_wins_over_whitelist() {
        let config = test_config(
            vec![HostnameMatcher::Suffix("iqiyi.com".to_string())],
            vec![HostnameMatcher::Suffix("vip.iqiyi.com".to_string())],
        );
        assert!(
            !should_intercept_host(&config, "vip.iqiyi.com"),
            "SNI 排除命中即使白名单命中也不应拦截"
        );
    }

    #[test]
    fn should_intercept_tls_empty_sni_falls_back_to_library_semantics() {
        // 白名单非空：无 SNI（空主机名）不匹配任何白名单条目，不拦截。
        let whitelisted = test_config(
            vec![HostnameMatcher::Suffix("iqiyi.com".to_string())],
            Vec::new(),
        );
        assert!(
            !should_intercept_host(&whitelisted, ""),
            "白名单非空且无 SNI 时不应拦截"
        );
        // 白名单为空：保持「空=全拦截」库语义，无 SNI 仍拦截。
        let open = test_config(Vec::new(), Vec::new());
        assert!(
            should_intercept_host(&open, ""),
            "白名单为空时无 SNI 仍全量拦截（库语义不变）"
        );
    }
}
