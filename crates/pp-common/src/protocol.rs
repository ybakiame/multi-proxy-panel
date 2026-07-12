use serde::{Deserialize, Serialize};

/// Supported proxy protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolType {
    VlessReality,
    VlessXhttp,
    Hysteria2,
    Anytls,
}

impl std::fmt::Display for ProtocolType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            ProtocolType::VlessReality => "vless_reality",
            ProtocolType::VlessXhttp => "vless_xhttp",
            ProtocolType::Hysteria2 => "hysteria2",
            ProtocolType::Anytls => "anytls",
        };
        write!(f, "{}", s)
    }
}

impl std::str::FromStr for ProtocolType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "vless_reality" => Ok(ProtocolType::VlessReality),
            "vless_xhttp" => Ok(ProtocolType::VlessXhttp),
            "hysteria2" => Ok(ProtocolType::Hysteria2),
            "anytls" => Ok(ProtocolType::Anytls),
            _ => Err(format!("unknown protocol type: {}", s)),
        }
    }
}

/// Which core(s) a protocol config targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CoreType {
    #[default]
    Xray,
    SingBox,
}

impl CoreType {
    pub fn valid_for(protocol: ProtocolType) -> &'static [CoreType] {
        use {CoreType::*, ProtocolType::*};
        match protocol {
            VlessReality => &[Xray, SingBox],
            VlessXhttp => &[Xray],
            Hysteria2 | Anytls => &[SingBox],
        }
    }
}

impl std::fmt::Display for CoreType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoreType::Xray => write!(f, "xray"),
            CoreType::SingBox => write!(f, "sing-box"),
        }
    }
}

impl std::str::FromStr for CoreType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "xray" => Ok(CoreType::Xray),
            "sing-box" | "singbox" => Ok(CoreType::SingBox),
            _ => Err(format!("unknown core type: {}", s)),
        }
    }
}

/// Node / Agent status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    Connecting,
    Online,
    Degraded,
    Offline,
}

/// User account status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserStatus {
    Active,
    Disabled,
    Limited,
    Expired,
    OnHold,
}

/// Traffic unit for display and quotas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrafficUnit {
    Bytes,
    KiB,
    MiB,
    GiB,
    TiB,
}

impl TrafficUnit {
    pub fn to_bytes(&self, value: u64) -> u64 {
        match self {
            TrafficUnit::Bytes => value,
            TrafficUnit::KiB => value * 1024,
            TrafficUnit::MiB => value * 1024 * 1024,
            TrafficUnit::GiB => value * 1024 * 1024 * 1024,
            TrafficUnit::TiB => value * 1024 * 1024 * 1024 * 1024,
        }
    }
}
