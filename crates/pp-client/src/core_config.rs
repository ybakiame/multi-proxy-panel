//! 核心配置合成：将订阅配置合成为本地可用的核心启动配置。

use pp_common::{PanelError, PanelResult};
use serde_json::{Value, json};

/// 将订阅配置合成为可直接传给 sing-box 的本地配置。
///
/// 入站整体替换为单个 mixed 入站（仅监听 `127.0.0.1`），
/// `outbounds` / `route` / `dns` / `log` 等其余字段原样保留。
pub fn compose_singbox_config(sub_config: &Value, mixed_port: u16) -> PanelResult<Value> {
    let mut obj = sub_config.as_object().cloned().ok_or_else(|| {
        PanelError::Client("subscription config must be a JSON object".to_string())
    })?;

    let mixed_inbound = json!({
        "type": "mixed",
        "tag": "mixed-in",
        "listen": "127.0.0.1",
        "listen_port": mixed_port,
    });
    obj.insert("inbounds".to_string(), Value::Array(vec![mixed_inbound]));
    Ok(Value::Object(obj))
}

/// 合成 mihomo 配置（尚未支持）。
pub fn compose_mihomo_config(_sub_config: &Value, _mixed_port: u16) -> PanelResult<Value> {
    Err(PanelError::Client("mihomo not yet supported".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_subscription() -> Value {
        json!({
            "inbounds": [{ "type": "vless", "tag": "hub-in", "listen_port": 443 }],
            "outbounds": [
                { "type": "vless", "tag": "n1", "server": "example.com", "server_port": 443 },
                { "type": "direct", "tag": "direct" }
            ],
            "route": { "final": "n1" },
            "dns": { "servers": ["1.1.1.1"] },
            "log": { "level": "info" }
        })
    }

    #[test]
    fn compose_singbox_replaces_inbounds_and_preserves_rest() {
        let sub = sample_subscription();
        let cfg = compose_singbox_config(&sub, 17890).unwrap();

        let inbounds = cfg["inbounds"].as_array().unwrap();
        assert_eq!(inbounds.len(), 1);
        assert_eq!(inbounds[0]["type"], "mixed");
        assert_eq!(inbounds[0]["tag"], "mixed-in");
        assert_eq!(inbounds[0]["listen"], "127.0.0.1");
        assert_eq!(inbounds[0]["listen_port"], 17890);

        assert_eq!(cfg["outbounds"], sub["outbounds"]);
        assert_eq!(cfg["route"], sub["route"]);
        assert_eq!(cfg["dns"], sub["dns"]);
        assert_eq!(cfg["log"], sub["log"]);
    }

    #[test]
    fn compose_singbox_rejects_non_object() {
        let err = compose_singbox_config(&json!([1, 2, 3]), 17890).unwrap_err();
        assert!(matches!(err, PanelError::Client(_)));
    }

    #[test]
    fn compose_mihomo_returns_unsupported_error() {
        let err = compose_mihomo_config(&sample_subscription(), 17890).unwrap_err();
        assert!(matches!(err, PanelError::Client(_)));
    }
}
