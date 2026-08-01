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

/// 将 clash YAML 订阅配置合成为 mihomo 本地启动配置。
///
/// 解析 YAML 后注入本地 mixed 入口：写入 `mixed-port`，移除冲突的
/// `port` / `socks-port` / `redir-port` / `tproxy-port`（客户端只保留
/// mixed 入站），并以 `allow-lan: false`、`bind-address: 127.0.0.1` 作为
/// 安全默认。`proxies` / `proxy-groups` / `rules` / `dns` 等其余字段原样保留。
pub fn compose_mihomo_config(sub_yaml: &str, mixed_port: u16) -> PanelResult<Value> {
    let value: Value = serde_yaml::from_str(sub_yaml)
        .map_err(|e| PanelError::Client(format!("invalid clash config in subscription: {e}")))?;
    let mut obj = value
        .as_object()
        .cloned()
        .ok_or_else(|| PanelError::Client("clash config must be a YAML mapping".to_string()))?;

    for key in ["port", "socks-port", "redir-port", "tproxy-port"] {
        obj.remove(key);
    }
    obj.insert("mixed-port".to_string(), Value::from(mixed_port));
    obj.insert("allow-lan".to_string(), Value::Bool(false));
    obj.insert(
        "bind-address".to_string(),
        Value::String("127.0.0.1".to_string()),
    );
    Ok(Value::Object(obj))
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
    fn compose_mihomo_injects_mixed_and_removes_conflicting_ports() {
        let yaml = r#"
port: 7890
socks-port: 7891
redir-port: 7892
tproxy-port: 7893
proxies:
  - name: n1
    type: vless
    server: example.com
    port: 443
proxy-groups:
  - name: PROXY
    type: select
    proxies: [n1]
rules:
  - DOMAIN-SUFFIX,example.com,PROXY
dns:
  enable: true
"#;
        let cfg = compose_mihomo_config(yaml, 17890).unwrap();

        // mixed 入口注入，冲突的独立端口字段移除。
        assert_eq!(cfg["mixed-port"], 17890);
        assert!(cfg.get("port").is_none());
        assert!(cfg.get("socks-port").is_none());
        assert!(cfg.get("redir-port").is_none());
        assert!(cfg.get("tproxy-port").is_none());

        // 安全默认。
        assert_eq!(cfg["allow-lan"], false);
        assert_eq!(cfg["bind-address"], "127.0.0.1");

        // 其余字段原样保留（含 proxies 条目内部的 server port）。
        assert_eq!(cfg["proxies"][0]["server"], "example.com");
        assert_eq!(cfg["proxies"][0]["port"], 443);
        assert_eq!(cfg["proxy-groups"][0]["name"], "PROXY");
        assert_eq!(cfg["rules"][0], "DOMAIN-SUFFIX,example.com,PROXY");
        assert_eq!(cfg["dns"]["enable"], true);
    }

    #[test]
    fn compose_mihomo_overrides_existing_mixed_port() {
        let yaml = "mixed-port: 7890\nallow-lan: true\nproxies:\n  - name: n1\n    type: direct\n";
        let cfg = compose_mihomo_config(yaml, 17890).unwrap();

        assert_eq!(cfg["mixed-port"], 17890);
        assert_eq!(cfg["allow-lan"], false);
        assert_eq!(cfg["bind-address"], "127.0.0.1");
    }

    #[test]
    fn compose_mihomo_rejects_invalid_yaml() {
        let err = compose_mihomo_config("port: [unclosed", 17890).unwrap_err();
        assert!(matches!(err, PanelError::Client(_)));
    }

    #[test]
    fn compose_mihomo_rejects_non_mapping_yaml() {
        let err = compose_mihomo_config("- a\n- b", 17890).unwrap_err();
        assert!(matches!(err, PanelError::Client(_)));
    }
}
