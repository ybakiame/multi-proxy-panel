//! Hub configuration loaded from file and environment.

use std::collections::HashSet;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct HubConfig {
    pub listen: String,
    pub grpc_listen: String,
    pub database_url: String,
    pub static_dir: PathBuf,
    pub cors_origins: Option<Vec<String>>,
    pub trusted_proxy_ips: Option<HashSet<IpAddr>>,
    pub auto_register_agents: bool,
    pub jwt_secret: String,
    pub http_tls_cert: Option<PathBuf>,
    pub http_tls_key: Option<PathBuf>,
}

impl Default for HubConfig {
    fn default() -> Self {
        Self {
            listen: "0.0.0.0:8081".to_string(),
            grpc_listen: "0.0.0.0:50052".to_string(),
            database_url: String::new(),
            static_dir: PathBuf::from("crates/pp-web/dist"),
            cors_origins: None,
            trusted_proxy_ips: None,
            auto_register_agents: false,
            jwt_secret: "change-me-to-a-random-secret".to_string(),
            http_tls_cert: None,
            http_tls_key: None,
        }
    }
}

impl HubConfig {
    pub fn load(path: &Path, overrides: ConfigOverrides) -> Result<Self, config::ConfigError> {
        let mut cfg = config::Config::builder();

        if path.exists() {
            cfg = cfg.add_source(config::File::from(path));
        }

        cfg = cfg
            .add_source(config::Environment::with_prefix("PROXYPANEL").separator("_"))
            .set_default("listen", Self::default().listen)?
            .set_default("grpc_listen", Self::default().grpc_listen)?
            .set_default(
                "static_dir",
                Self::default().static_dir.to_string_lossy().to_string(),
            )?
            .set_default("auto_register_agents", false)?
            .set_default("jwt_secret", Self::default().jwt_secret)?;

        let built = cfg.build()?;

        let mut hub_config = Self {
            listen: built.get_string("listen")?,
            grpc_listen: built.get_string("grpc_listen")?,
            database_url: overrides
                .database_url
                .unwrap_or_else(|| built.get_string("database_url").unwrap_or_default()),
            static_dir: built
                .get_string("static_dir")
                .unwrap_or_else(|_| Self::default().static_dir.to_string_lossy().to_string())
                .into(),
            cors_origins: parse_csv(built.get_string("cors_origins").ok().as_deref()),
            trusted_proxy_ips: parse_ips(built.get_string("trusted_proxy_ips").ok().as_deref()),
            auto_register_agents: built.get_bool("auto_register_agents").unwrap_or(false),
            jwt_secret: built
                .get_string("jwt_secret")
                .unwrap_or_else(|_| Self::default().jwt_secret),
            http_tls_cert: built.get_string("http_tls_cert").ok().map(PathBuf::from),
            http_tls_key: built.get_string("http_tls_key").ok().map(PathBuf::from),
        };

        if let Some(listen) = overrides.listen {
            hub_config.listen = listen;
        }
        if let Some(grpc_listen) = overrides.grpc_listen {
            hub_config.grpc_listen = grpc_listen;
        }
        if let Some(static_dir) = overrides.static_dir {
            hub_config.static_dir = static_dir;
        }
        if let Some(auto) = overrides.auto_register_agents {
            hub_config.auto_register_agents = auto;
        }

        Ok(hub_config)
    }

    pub fn cors_allowed_origins(&self) -> Option<Vec<axum::http::HeaderValue>> {
        self.cors_origins.as_ref().map(|origins| {
            origins
                .iter()
                .filter_map(|o| axum::http::HeaderValue::from_str(o).ok())
                .collect()
        })
    }
}

#[derive(Default)]
pub struct ConfigOverrides {
    pub listen: Option<String>,
    pub grpc_listen: Option<String>,
    pub database_url: Option<String>,
    pub static_dir: Option<PathBuf>,
    pub auto_register_agents: Option<bool>,
}

fn parse_csv(value: Option<&str>) -> Option<Vec<String>> {
    value.and_then(|v| {
        let items: Vec<String> = v
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if items.is_empty() { None } else { Some(items) }
    })
}

fn parse_ips(value: Option<&str>) -> Option<HashSet<IpAddr>> {
    value.and_then(|v| {
        let ips: HashSet<IpAddr> = v
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        if ips.is_empty() { None } else { Some(ips) }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_csv_works() {
        assert_eq!(
            parse_csv(Some("https://a.com, http://b.com")),
            Some(vec![
                "https://a.com".to_string(),
                "http://b.com".to_string()
            ])
        );
        assert_eq!(parse_csv(Some("")), None);
        assert_eq!(parse_csv(None), None);
    }

    #[test]
    fn parse_ips_works() {
        let ips = parse_ips(Some("127.0.0.1, ::1")).unwrap();
        assert!(ips.contains(&"127.0.0.1".parse().unwrap()));
        assert!(ips.contains(&"::1".parse().unwrap()));
    }
}
