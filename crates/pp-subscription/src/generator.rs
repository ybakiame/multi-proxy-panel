use pp_common::{PanelError, PanelResult, ProtocolType};
use serde_json::Value;

/// Supported subscription output formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionFormat {
    Base64,
    Json,
    Clash,
    SingBox,
    V2RayNG,
}

impl std::str::FromStr for SubscriptionFormat {
    type Err = PanelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "base64" | "default" => Ok(SubscriptionFormat::Base64),
            "json" => Ok(SubscriptionFormat::Json),
            "clash" | "yaml" => Ok(SubscriptionFormat::Clash),
            "sing-box" | "singbox" => Ok(SubscriptionFormat::SingBox),
            "v2rayng" => Ok(SubscriptionFormat::V2RayNG),
            _ => Err(PanelError::Subscription(format!(
                "unknown subscription format: {}",
                s
            ))),
        }
    }
}

/// A normalized proxy node description used by generators.
#[derive(Debug, Clone)]
pub struct ProxyNode {
    pub name: String,
    pub protocol: ProtocolType,
    pub server: String,
    pub port: u16,
    pub settings: Value,
    pub tls: Option<Value>,
}

/// Generate subscription content for a given format.
/// `base_config` is the raw template content (YAML for Clash, JSON for Sing-box).
pub fn generate_subscription(
    format: SubscriptionFormat,
    nodes: &[ProxyNode],
    base_config: Option<&str>,
) -> PanelResult<String> {
    match format {
        SubscriptionFormat::Base64 => crate::formats::base64::generate(nodes),
        SubscriptionFormat::Json => crate::formats::json::generate(nodes),
        SubscriptionFormat::Clash => crate::formats::clash::generate(nodes, base_config),
        SubscriptionFormat::SingBox => crate::formats::singbox::generate(nodes, base_config),
        SubscriptionFormat::V2RayNG => crate::formats::v2rayng::generate(nodes),
    }
}
