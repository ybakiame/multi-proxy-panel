//! 分享链接解析：ss / vmess / vless / trojan / hysteria2 / tuic / anytls。
//!
//! 按行解析，每条链接同时产出 sing-box outbound 与 clash/mihomo proxy 双核心节点。
//! 单行解析失败跳过并记 warning；非分享链接行（无 `://` 或未知 scheme）静默跳过。

use std::collections::HashMap;

use base64::Engine as _;
use pp_common::{PanelError, PanelResult};
use serde_json::{Map, Value, json};

use crate::node_convert::singbox_to_mihomo;

/// 单个解析出的分享链接节点（双核心表示）。
#[derive(Debug, Clone)]
pub struct ShareNode {
    /// 节点名称（分享链接 `#name`，缺省为 `host:port`）。
    pub name: String,
    /// sing-box outbound（含 `tag`）。
    pub outbound_singbox: Value,
    /// clash/mihomo proxy（含 `name`）。
    pub proxy_mihomo: Value,
}

/// 分享链接解析结果：节点列表 + 每行失败时的 warning。
#[derive(Debug, Clone, Default)]
pub struct ShareLinkParseResult {
    pub nodes: Vec<ShareNode>,
    pub warnings: Vec<String>,
}

/// 按行解析分享链接文本。单行失败跳过并记 warning；
/// 非分享链接行（无 `://` 或未知 scheme）静默跳过。
pub fn parse_share_links(content: &str) -> ShareLinkParseResult {
    let mut result = ShareLinkParseResult::default();
    for (idx, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match parse_share_link(line) {
            Ok(Some(node)) => result.nodes.push(node),
            Ok(None) => {}
            Err(e) => result.warnings.push(format!("line {}: {e}", idx + 1)),
        }
    }
    result
}

/// 解析单条分享链接。返回 `Ok(None)` 表示该行不是本模块支持的分享链接。
fn parse_share_link(line: &str) -> PanelResult<Option<ShareNode>> {
    let (scheme, rest) = match line.split_once("://") {
        Some(s) => s,
        None => return Ok(None),
    };
    let outbound = match scheme {
        "ss" => parse_ss(rest),
        "vmess" => parse_vmess(rest),
        "vless" => parse_vless(rest),
        "trojan" => parse_trojan(rest),
        "hysteria2" | "hy2" => parse_hysteria2(rest),
        "tuic" => parse_tuic(rest),
        "anytls" => parse_anytls(rest),
        _ => return Ok(None),
    };
    let outbound = outbound
        .ok_or_else(|| PanelError::Client(format!("failed to parse {scheme}:// share link")))?;
    let name = outbound["tag"].as_str().unwrap_or_default().to_string();
    let proxy_mihomo = singbox_to_mihomo(&outbound).ok_or_else(|| {
        PanelError::Client(format!("unsupported share link protocol: {scheme}://"))
    })?;
    Ok(Some(ShareNode {
        name,
        outbound_singbox: outbound,
        proxy_mihomo,
    }))
}

/// `ss://`（SIP002：base64(method:password)@host:port，或旧式整体 base64）。
fn parse_ss(rest: &str) -> Option<Value> {
    let (body, frag) = split_fragment(rest);
    let (authority, _query) = split_query(body);
    let name = percent_decode(frag);

    // 旧式整体 base64：method:password@host:port。含 '@' 时直接按明文处理。
    let decoded_legacy: String;
    let userinfo_hostport: &str = if authority.contains('@') {
        authority
    } else {
        decoded_legacy = b64_decode(authority)?;
        decoded_legacy.as_str()
    };

    let (userinfo, hostport) = userinfo_hostport.split_once('@')?;
    let (server, port) = parse_host_port(hostport)?;

    // SIP002 的 userinfo 是 base64(method:password)；含 ':' 时按明文处理。
    let decoded_cred: String;
    let cred: &str = if userinfo.contains(':') {
        userinfo
    } else {
        match b64_decode(userinfo) {
            Some(d) => {
                decoded_cred = d;
                decoded_cred.as_str()
            }
            None => userinfo,
        }
    };
    let (method, password) = cred.split_once(':')?;

    let mut o = json!({
        "type": "shadowsocks",
        "server": server,
        "server_port": port,
        "method": method,
        "password": password,
    });
    o["tag"] = json!(fallback_name(&name, &server, port));
    Some(o)
}

/// `vmess://` base64 JSON（v/ps/add/port/id/aid/net/type/host/path/tls/sni/fp）。
fn parse_vmess(rest: &str) -> Option<Value> {
    let (body, _frag) = split_fragment(rest);
    let body = body.trim();
    let json_str = if body.starts_with('{') {
        body.to_string()
    } else {
        b64_decode(body)?
    };
    let v: Value = serde_json::from_str(&json_str).ok()?;

    let add = v.get("add").and_then(Value::as_str)?.to_string();
    let port = port_of(v.get("port"))?;
    let id = v.get("id").and_then(Value::as_str)?.to_string();
    let name = v
        .get("ps")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .unwrap_or_else(|| format!("{add}:{port}"));
    let net = v.get("net").and_then(Value::as_str).unwrap_or("tcp");
    let host = v.get("host").and_then(Value::as_str).unwrap_or_default();
    let path = v.get("path").and_then(Value::as_str).unwrap_or_default();
    let tls_enabled = v
        .get("tls")
        .and_then(Value::as_str)
        .is_some_and(|s| s == "tls" || s == "1");
    let sni = v
        .get("sni")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty());
    let fp = v
        .get("fp")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty());
    let scy = v
        .get("scy")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty());
    let aid = as_u64(v.get("aid")).unwrap_or(0);

    let mut o = json!({
        "type": "vmess",
        "server": add,
        "server_port": port,
        "uuid": id,
        "alter_id": aid,
        "security": scy.unwrap_or("auto"),
    });
    o["tag"] = json!(name);
    if tls_enabled {
        let mut tls = Map::new();
        tls.insert(String::from("enabled"), json!(true));
        let server_name = sni
            .filter(|s| !s.is_empty())
            .or_else(|| (!host.is_empty()).then_some(host));
        if let Some(sn) = server_name {
            tls.insert(String::from("server_name"), json!(sn));
        }
        if let Some(f) = fp {
            tls.insert(
                String::from("utls"),
                json!({ "enabled": true, "fingerprint": f }),
            );
        }
        o["tls"] = Value::Object(tls);
    }
    if let Some(t) = build_vmess_transport(net, host, path) {
        o["transport"] = t;
    }
    Some(o)
}

/// `vless://` uuid@host:port 带 query（security/flow/sni/fp/pbk/sid/type/path/serviceName）。
fn parse_vless(rest: &str) -> Option<Value> {
    let (body, frag) = split_fragment(rest);
    let (authority, query) = split_query(body);
    let name = percent_decode(frag);
    let (uuid, hostport) = authority.split_once('@')?;
    let (server, port) = parse_host_port(hostport)?;
    let params = parse_query(query);

    let security = params.get("security").map(String::as_str).unwrap_or("");
    let tls_enabled = security == "tls" || security == "reality";
    let flow = params.get("flow").filter(|s| !s.is_empty());
    let sni = params.get("sni").filter(|s| !s.is_empty());
    let fp = params.get("fp").filter(|s| !s.is_empty());
    let pbk = params.get("pbk").filter(|s| !s.is_empty());
    let sid = params.get("sid").filter(|s| !s.is_empty());
    let net = params.get("type").map(String::as_str).unwrap_or("tcp");
    let path = params.get("path").filter(|s| !s.is_empty());
    let service_name = params.get("serviceName").filter(|s| !s.is_empty());
    let ws_host = params.get("host").filter(|s| !s.is_empty());

    let mut o = json!({
        "type": "vless",
        "server": server,
        "server_port": port,
        "uuid": uuid,
        "packet_encoding": "xudp",
    });
    o["tag"] = json!(fallback_name(&name, &server, port));
    if let Some(f) = flow {
        o["flow"] = json!(f);
    }
    if tls_enabled {
        let mut tls = Map::new();
        tls.insert(String::from("enabled"), json!(true));
        if let Some(s) = sni {
            tls.insert(String::from("server_name"), json!(s));
        }
        if let Some(f) = fp {
            tls.insert(
                String::from("utls"),
                json!({ "enabled": true, "fingerprint": f }),
            );
        }
        if security == "reality" {
            let mut r = Map::new();
            r.insert(String::from("enabled"), json!(true));
            if let Some(p) = pbk {
                r.insert(String::from("public_key"), json!(p));
            }
            if let Some(s) = sid {
                r.insert(String::from("short_id"), json!(s));
            }
            tls.insert(String::from("reality"), Value::Object(r));
        }
        o["tls"] = Value::Object(tls);
    }
    match net {
        "ws" => {
            let mut t = Map::new();
            t.insert(String::from("type"), json!("ws"));
            if let Some(p) = path {
                t.insert(String::from("path"), json!(p));
            }
            if let Some(h) = ws_host {
                t.insert(String::from("headers"), json!({ "Host": h }));
            }
            o["transport"] = Value::Object(t);
        }
        "grpc" => {
            let mut t = Map::new();
            t.insert(String::from("type"), json!("grpc"));
            if let Some(sn) = service_name {
                t.insert(String::from("service_name"), json!(sn));
            }
            o["transport"] = Value::Object(t);
        }
        "http" | "h2" => {
            let mut t = Map::new();
            t.insert(String::from("type"), json!("http"));
            if let Some(p) = path {
                t.insert(String::from("path"), json!(p));
            }
            o["transport"] = Value::Object(t);
        }
        _ => {}
    }
    Some(o)
}

/// `trojan://` password@host:port?sni&allowInsecure#name。
fn parse_trojan(rest: &str) -> Option<Value> {
    let (body, frag) = split_fragment(rest);
    let (authority, query) = split_query(body);
    let name = percent_decode(frag);
    let (password, hostport) = authority.split_once('@')?;
    let (server, port) = parse_host_port(hostport)?;
    let params = parse_query(query);
    let sni = params.get("sni").filter(|s| !s.is_empty());
    let allow_insecure = truthy(
        params
            .get("allowInsecure")
            .or_else(|| params.get("insecure")),
    );

    let mut o = json!({
        "type": "trojan",
        "server": server,
        "server_port": port,
        "password": password,
        "tls": { "enabled": true },
    });
    o["tag"] = json!(fallback_name(&name, &server, port));
    if let Some(s) = sni {
        o["tls"]["server_name"] = json!(s);
    }
    if allow_insecure {
        o["tls"]["insecure"] = json!(true);
    }
    Some(o)
}

/// `hysteria2://` / `hy2://` password@host:port?sni&insecure#name。
fn parse_hysteria2(rest: &str) -> Option<Value> {
    let (body, frag) = split_fragment(rest);
    let (authority, query) = split_query(body);
    let name = percent_decode(frag);
    let (password, hostport) = authority.split_once('@')?;
    let (server, port) = parse_host_port(hostport)?;
    let params = parse_query(query);
    let sni = params.get("sni").filter(|s| !s.is_empty());
    let insecure = truthy(params.get("insecure"));

    let mut o = json!({
        "type": "hysteria2",
        "server": server,
        "server_port": port,
        "password": password,
        "tls": { "enabled": true },
    });
    o["tag"] = json!(fallback_name(&name, &server, port));
    if let Some(s) = sni {
        o["tls"]["server_name"] = json!(s);
    }
    if insecure {
        o["tls"]["insecure"] = json!(true);
    }
    Some(o)
}

/// `tuic://` uuid:password@host:port?sni&congestion_control#name。
fn parse_tuic(rest: &str) -> Option<Value> {
    let (body, frag) = split_fragment(rest);
    let (authority, query) = split_query(body);
    let name = percent_decode(frag);
    let (uuid_password, hostport) = authority.split_once('@')?;
    let (uuid, password) = uuid_password.split_once(':')?;
    let (server, port) = parse_host_port(hostport)?;
    let params = parse_query(query);
    let sni = params.get("sni").filter(|s| !s.is_empty());
    let cc = params.get("congestion_control").filter(|s| !s.is_empty());

    let mut o = json!({
        "type": "tuic",
        "server": server,
        "server_port": port,
        "uuid": uuid,
        "password": password,
        "tls": { "enabled": true },
    });
    o["tag"] = json!(fallback_name(&name, &server, port));
    if let Some(s) = sni {
        o["tls"]["server_name"] = json!(s);
    }
    if let Some(c) = cc {
        o["congestion_control"] = json!(c);
    }
    Some(o)
}

/// `anytls://` password@host:port?sni#name。
fn parse_anytls(rest: &str) -> Option<Value> {
    let (body, frag) = split_fragment(rest);
    let (authority, query) = split_query(body);
    let name = percent_decode(frag);
    let (password, hostport) = authority.split_once('@')?;
    let (server, port) = parse_host_port(hostport)?;
    let params = parse_query(query);
    let sni = params.get("sni").filter(|s| !s.is_empty());

    let mut o = json!({
        "type": "anytls",
        "server": server,
        "server_port": port,
        "password": password,
        "tls": { "enabled": true },
    });
    o["tag"] = json!(fallback_name(&name, &server, port));
    if let Some(s) = sni {
        o["tls"]["server_name"] = json!(s);
    }
    Some(o)
}

// ---------- 通用工具 ----------

fn split_fragment(s: &str) -> (&str, &str) {
    match s.split_once('#') {
        Some((a, b)) => (a, b),
        None => (s, ""),
    }
}

fn split_query(s: &str) -> (&str, &str) {
    match s.split_once('?') {
        Some((a, b)) => (a, b),
        None => (s, ""),
    }
}

fn parse_query(s: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if s.is_empty() {
        return map;
    }
    for pair in s.split('&') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        match pair.split_once('=') {
            Some((k, v)) => {
                map.insert(percent_decode(k), percent_decode(v));
            }
            None => {
                map.insert(percent_decode(pair), String::new());
            }
        }
    }
    map
}

/// 解析 `host:port`；IPv6 支持 `[::1]:443`。
fn parse_host_port(s: &str) -> Option<(String, u16)> {
    let s = s.trim();
    if let Some(inner) = s.strip_prefix('[') {
        let (host, rest) = inner.split_once(']')?;
        let port = rest.strip_prefix(':')?.trim();
        let port: u16 = port.parse().ok()?;
        return Some((host.to_string(), port));
    }
    let (host, port) = s.rsplit_once(':')?;
    let port: u16 = port.trim().parse().ok()?;
    Some((host.to_string(), port))
}

fn fallback_name(name: &str, server: &str, port: u16) -> String {
    if name.is_empty() {
        format!("{server}:{port}")
    } else {
        name.to_string()
    }
}

/// 支持标准与 URL-safe 两种 base64 字母表，容忍缺失 padding。
pub(crate) fn b64_decode(s: &str) -> Option<String> {
    let s = s.trim();
    let mut padded = s.to_string();
    while padded.len() % 4 != 0 {
        padded.push('=');
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(padded.as_bytes())
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(padded.as_bytes()))
        .ok()?;
    String::from_utf8(bytes).ok()
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex_val(b[i + 1]), hex_val(b[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

fn port_of(v: Option<&Value>) -> Option<u16> {
    let v = v?;
    if let Some(n) = v.as_u64() {
        return u16::try_from(n).ok();
    }
    if let Some(n) = v.as_i64() {
        return u16::try_from(n).ok();
    }
    v.as_str()?.parse().ok()
}

fn as_u64(v: Option<&Value>) -> Option<u64> {
    let v = v?;
    if let Some(n) = v.as_u64() {
        return Some(n);
    }
    if let Some(n) = v.as_i64() {
        return u64::try_from(n).ok();
    }
    v.as_str()?.parse().ok()
}

/// 把 "1" / "true" / "yes" / "on" 视为 true。
fn truthy(v: Option<&String>) -> bool {
    match v.map(String::as_str) {
        Some(s) => matches!(s.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"),
        None => false,
    }
}

/// vmess 的传输层构建（ws / grpc / http，其余视为 tcp 不输出）。
fn build_vmess_transport(net: &str, host: &str, path: &str) -> Option<Value> {
    match net {
        "ws" => {
            let mut t = Map::new();
            t.insert(String::from("type"), json!("ws"));
            if !path.is_empty() {
                t.insert(String::from("path"), json!(path));
            }
            if !host.is_empty() {
                t.insert(String::from("headers"), json!({ "Host": host }));
            }
            Some(Value::Object(t))
        }
        "grpc" => {
            let mut t = Map::new();
            t.insert(String::from("type"), json!("grpc"));
            if !path.is_empty() {
                t.insert(String::from("service_name"), json!(path));
            }
            if !host.is_empty() {
                t.insert(String::from("authority"), json!(host));
            }
            Some(Value::Object(t))
        }
        "http" | "h2" | "httpupgrade" => {
            let mut t = Map::new();
            t.insert(String::from("type"), json!("http"));
            if !path.is_empty() {
                t.insert(String::from("path"), json!(path));
            }
            if !host.is_empty() {
                t.insert(String::from("host"), json!(host));
            }
            Some(Value::Object(t))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UUID: &str = "12345678-1234-1234-1234-123456789012";

    fn b64(s: &str) -> String {
        base64::engine::general_purpose::STANDARD.encode(s)
    }

    /// 解析单条链接并返回 sing-box outbound（测试便捷入口）。
    fn parse_one(line: &str) -> Value {
        let result = parse_share_links(line);
        assert!(
            result.warnings.is_empty(),
            "unexpected warnings: {:?}",
            result.warnings
        );
        assert_eq!(result.nodes.len(), 1, "expected exactly one node");
        result.nodes[0].outbound_singbox.clone()
    }

    // ---------- ① 每协议至少一条样例链接 ----------

    #[test]
    fn parses_ss_sip002() {
        let link = format!(
            "ss://{}@example.com:8388#ss-node",
            b64("aes-256-gcm:password")
        );
        let o = parse_one(&link);
        assert_eq!(o["type"], "shadowsocks");
        assert_eq!(o["server"], "example.com");
        assert_eq!(o["server_port"], 8388);
        assert_eq!(o["method"], "aes-256-gcm");
        assert_eq!(o["password"], "password");
        assert_eq!(o["tag"], "ss-node");
    }

    #[test]
    fn parses_ss_legacy_whole_base64() {
        let link = format!(
            "ss://{}#legacy-node",
            b64("aes-256-gcm:password@example.com:8388")
        );
        let o = parse_one(&link);
        assert_eq!(o["type"], "shadowsocks");
        assert_eq!(o["server"], "example.com");
        assert_eq!(o["server_port"], 8388);
        assert_eq!(o["method"], "aes-256-gcm");
        assert_eq!(o["password"], "password");
        assert_eq!(o["tag"], "legacy-node");
    }

    #[test]
    fn parses_ss_plaintext_userinfo() {
        let o = parse_one("ss://aes-256-gcm:password@example.com:8388#plain");
        assert_eq!(o["server"], "example.com");
        assert_eq!(o["server_port"], 8388);
        assert_eq!(o["method"], "aes-256-gcm");
        assert_eq!(o["password"], "password");
    }

    #[test]
    fn parses_vmess() {
        let v = json!({
            "v": "2", "ps": "vmess-node", "add": "example.com", "port": "443",
            "id": UUID, "aid": "0", "net": "ws", "type": "none",
            "host": "cdn.example.com", "path": "/ws", "tls": "tls",
            "sni": "example.com", "fp": "chrome"
        });
        let o = parse_one(&format!("vmess://{}", b64(&v.to_string())));
        assert_eq!(o["type"], "vmess");
        assert_eq!(o["server"], "example.com");
        assert_eq!(o["server_port"], 443);
        assert_eq!(o["uuid"], UUID);
        assert_eq!(o["tag"], "vmess-node");
        assert_eq!(o["tls"]["enabled"], true);
        assert_eq!(o["tls"]["server_name"], "example.com");
        assert_eq!(o["tls"]["utls"]["fingerprint"], "chrome");
        assert_eq!(o["transport"]["type"], "ws");
        assert_eq!(o["transport"]["path"], "/ws");
        assert_eq!(o["transport"]["headers"]["Host"], "cdn.example.com");
    }

    #[test]
    fn parses_vmess_plain_json() {
        let v = json!({
            "add": "example.com", "port": 443, "id": UUID, "ps": "vmess-plain",
            "net": "tcp", "tls": "tls", "sni": "example.com"
        });
        let o = parse_one(&format!("vmess://{v}"));
        assert_eq!(o["server"], "example.com");
        assert_eq!(o["server_port"], 443);
        assert_eq!(o["tls"]["server_name"], "example.com");
    }

    #[test]
    fn parses_vless_reality() {
        let link = format!(
            "vless://{UUID}@example.com:443?security=reality&sni=example.com&fp=chrome&pbk=REALITY_PK&sid=abcd1234&type=tcp#vless-reality"
        );
        let o = parse_one(&link);
        assert_eq!(o["type"], "vless");
        assert_eq!(o["server"], "example.com");
        assert_eq!(o["server_port"], 443);
        assert_eq!(o["uuid"], UUID);
        assert_eq!(o["tag"], "vless-reality");
        assert_eq!(o["tls"]["enabled"], true);
        assert_eq!(o["tls"]["server_name"], "example.com");
        assert_eq!(o["tls"]["utls"]["fingerprint"], "chrome");
        assert_eq!(o["tls"]["reality"]["public_key"], "REALITY_PK");
        assert_eq!(o["tls"]["reality"]["short_id"], "abcd1234");
    }

    #[test]
    fn parses_vless_ws() {
        let link = format!(
            "vless://{UUID}@example.com:443?security=tls&sni=example.com&type=ws&path=%2Fws&host=cdn.example.com#vless-ws"
        );
        let o = parse_one(&link);
        assert_eq!(o["server"], "example.com");
        assert_eq!(o["tls"]["server_name"], "example.com");
        assert_eq!(o["transport"]["type"], "ws");
        assert_eq!(o["transport"]["path"], "/ws");
        assert_eq!(o["transport"]["headers"]["Host"], "cdn.example.com");
    }

    #[test]
    fn parses_trojan() {
        let o = parse_one(
            "trojan://trojanpass@example.com:443?sni=example.com&allowInsecure=1#trojan-node",
        );
        assert_eq!(o["type"], "trojan");
        assert_eq!(o["server"], "example.com");
        assert_eq!(o["server_port"], 443);
        assert_eq!(o["password"], "trojanpass");
        assert_eq!(o["tls"]["enabled"], true);
        assert_eq!(o["tls"]["server_name"], "example.com");
        assert_eq!(o["tls"]["insecure"], true);
    }

    #[test]
    fn parses_hysteria2() {
        let o = parse_one("hysteria2://hypass@example.com:443?sni=example.com&insecure=1#hy2-node");
        assert_eq!(o["type"], "hysteria2");
        assert_eq!(o["server"], "example.com");
        assert_eq!(o["server_port"], 443);
        assert_eq!(o["password"], "hypass");
        assert_eq!(o["tls"]["server_name"], "example.com");
        assert_eq!(o["tls"]["insecure"], true);
    }

    #[test]
    fn parses_hy2_alias() {
        let o = parse_one("hy2://hypass@example.com:443?sni=example.com#hy2-alias");
        assert_eq!(o["type"], "hysteria2");
        assert_eq!(o["tag"], "hy2-alias");
    }

    #[test]
    fn parses_tuic() {
        let o = parse_one(&format!(
            "tuic://{UUID}:tuicpass@example.com:443?sni=example.com&congestion_control=bbr#tuic-node"
        ));
        assert_eq!(o["type"], "tuic");
        assert_eq!(o["server"], "example.com");
        assert_eq!(o["server_port"], 443);
        assert_eq!(o["uuid"], UUID);
        assert_eq!(o["password"], "tuicpass");
        assert_eq!(o["congestion_control"], "bbr");
        assert_eq!(o["tls"]["server_name"], "example.com");
    }

    #[test]
    fn parses_anytls() {
        let o = parse_one("anytls://anypass@example.com:443?sni=example.com#anytls-node");
        assert_eq!(o["type"], "anytls");
        assert_eq!(o["server"], "example.com");
        assert_eq!(o["server_port"], 443);
        assert_eq!(o["password"], "anypass");
        assert_eq!(o["tls"]["server_name"], "example.com");
    }

    // ---------- 双核心产出 ----------

    #[test]
    fn each_node_produces_both_singbox_and_mihomo() {
        let link = format!(
            "vless://{UUID}@example.com:443?security=reality&sni=example.com&fp=chrome&pbk=PK&sid=ab#dual"
        );
        let result = parse_share_links(&link);
        assert_eq!(result.nodes.len(), 1);
        let node = &result.nodes[0];
        assert_eq!(node.name, "dual");
        assert_eq!(node.outbound_singbox["type"], "vless");
        assert_eq!(node.outbound_singbox["tag"], "dual");
        assert_eq!(node.proxy_mihomo["type"], "vless");
        assert_eq!(node.proxy_mihomo["name"], "dual");
        assert_eq!(node.proxy_mihomo["server"], "example.com");
        assert_eq!(node.proxy_mihomo["port"], 443);
        assert_eq!(node.proxy_mihomo["servername"], "example.com");
        assert_eq!(node.proxy_mihomo["reality-opts"]["public-key"], "PK");
    }

    // ---------- 失败行 warning / 跳过 ----------

    #[test]
    fn malformed_line_produces_warning_and_is_skipped() {
        let content = format!(
            "vless://{UUID}@example.com:443?security=tls&sni=example.com#ok\n\
             trojan://broken-link-without-at\n\
             vless://{UUID}@example.com:443?security=tls&sni=example.com#also-ok"
        );
        let result = parse_share_links(&content);
        assert_eq!(result.nodes.len(), 2);
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("trojan"));
    }

    #[test]
    fn non_share_link_and_unknown_scheme_lines_are_skipped_silently() {
        let content = "random text line\n\
                       socks5://user:pass@example.com:1080\n\
                       vless://{UUID}@example.com:443?sni=example.com#real\n";
        let result = parse_share_links(content);
        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.nodes[0].name, "real");
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn parses_ipv6_host() {
        let link = format!("vless://{UUID}@[::1]:443?security=tls&sni=example.com#v6");
        let o = parse_one(&link);
        assert_eq!(o["server"], "::1");
        assert_eq!(o["server_port"], 443);
    }

    #[test]
    fn percent_decodes_fragment_names() {
        let o = parse_one("anytls://pw@example.com:443?sni=example.com#My%20Node%201");
        assert_eq!(o["tag"], "My Node 1");
    }
}
