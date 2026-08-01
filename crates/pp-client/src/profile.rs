//! Profile 层：订阅仅取节点，本地模板生成规则，支持 YAML 深合并复写 + JS 复写。
//!
//! 与 Clash Verge 的 Merge + Script 对齐：
//! - 订阅内容只用于提取代理节点（sing-box `outbounds` 叶子 / mihomo `proxies`），
//!   客户端实际运行配置由本地模板生成，避免订阅自带的分组 / 规则 / 路由覆盖本地。
//! - YAML 复写（[`apply_yaml_override`]）按 RFC 7386 深合并：对象递归合并，数组与
//!   标量整体替换。
//! - JS 复写（[`apply_js_override`]）为同步纯函数模式 `function main(config){...; return config}`，
//!   复用 pp-script QuickJS 引擎，宿主以 [`DenyHttpExecutor`] 拒绝一切网络、以内存
//!   存储承担持久化（无落盘），即"无网络 / 无存储权限"。
//!
//! 总装入口 [`build_core_config`]：提取节点 → 本地模板 → YAML 复写 → JS 复写。
//! inbounds 与 MITM 链不在本层，仍由 [`crate::state`] 通过
//! [`crate::core_config`] 的 `compose_*` 注入。

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use pp_common::{CoreType, PanelError, PanelResult};
use pp_script::{
    HttpExecutor, HttpRequestSpec, HttpResponseData, MemoryPersistentStore, QuickJsEngine,
    ScriptDialect, ScriptEngine, ScriptHost, ScriptKind, ScriptLimits,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::remote::TracingNotifier;

/// 订阅内容（两种核心的订阅原样），供 [`build_core_config`] 提取节点。
#[derive(Debug, Clone)]
pub enum SubContent {
    /// sing-box JSON 订阅配置。
    SingBox(Value),
    /// clash/mihomo YAML 订阅原文。
    Mihomo(String),
}

/// Profile 复写配置：空串 = 未启用。
///
/// 既作为旧版单文件存储 [`ProfileStore`] 的载荷，也作为多模板存储
/// [`ProfileStoreV2::active_for`] 的返回（取启用模板的复写字段）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileOverrides {
    /// YAML 深合并复写（RFC 7386 式；空串 = 未启用）。
    pub yaml_override: String,
    /// JS 复写（同步纯函数 `function main(config){...; return config}`；空串 = 未启用）。
    pub js_override: String,
}

/// 一个 Profile 模板：同一核心类型（[`CoreType`]）可维护多个，仅一条 `enabled`（排他）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    /// 模板唯一标识（应用层生成的 Uuid v4）。
    pub id: Uuid,
    /// 模板名称（存储内唯一，重名报错）。
    pub name: String,
    /// 目标核心类型（sing-box / mihomo）。
    pub core_type: CoreType,
    /// YAML 深合并复写（RFC 7386 式；空串 = 未启用）。
    pub yaml_override: String,
    /// JS 复写（同步纯函数 `function main(config){...; return config}`；空串 = 未启用）。
    pub js_override: String,
    /// 是否启用；同一核心类型下最多一条为 `true`。
    pub enabled: bool,
}

/// Profile 存储（旧版单文件）：读写 `data_dir/profile.json` 中的 [`ProfileOverrides`]。
///
/// 旧版单文件存储（仅兼容遗留调用方；新代码请使用多模板的 [`ProfileStoreV2`]）。
/// 其内容在 [`ProfileStoreV2::load`] 首次调用时一次性迁移到 `profiles.json` 后删除。
#[derive(Debug, Clone)]
pub struct ProfileStore {
    data_dir: PathBuf,
}

impl ProfileStore {
    /// 基于数据目录创建存储。
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    /// `data_dir/profile.json`。
    pub fn profile_file(&self) -> PathBuf {
        self.data_dir.join("profile.json")
    }

    /// 读取复写配置；文件缺失时返回默认（空），损坏时记 warning 并回退默认。
    pub fn load(&self) -> PanelResult<ProfileOverrides> {
        let path = self.profile_file();
        if !path.exists() {
            return Ok(ProfileOverrides::default());
        }
        let text = std::fs::read_to_string(&path)?;
        match serde_json::from_str(&text) {
            Ok(overrides) => Ok(overrides),
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "profile.json unreadable, fall back to defaults"
                );
                Ok(ProfileOverrides::default())
            }
        }
    }

    /// 保存复写配置到 `data_dir/profile.json`。
    pub fn save(&self, overrides: &ProfileOverrides) -> PanelResult<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        let text = serde_json::to_string_pretty(overrides)?;
        std::fs::write(self.profile_file(), text)?;
        Ok(())
    }
}

/// 多模板 Profile 存储：读写 `data_dir/profiles.json` 中的 [`Profile`] 列表。
///
/// 同一核心类型（[`CoreType`]）可维护多个模板，但仅一条 `enabled`（排他启用）；
/// 启动时经 [`ProfileStoreV2::active_for`] 取启用模板的复写。
///
/// 旧版单文件 `data_dir/profile.json`（[`ProfileStore`]）在首次 [`ProfileStoreV2::load`]
/// 时一次性迁移为 `Profile{name:"默认", core_type: SingBox, enabled:true}`（复写内容
/// 原样保留）并删除旧文件；`profiles.json` 已存在时不做迁移。
#[derive(Debug, Clone)]
pub struct ProfileStoreV2 {
    data_dir: PathBuf,
}

impl ProfileStoreV2 {
    /// 基于数据目录创建存储。
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    /// `data_dir/profiles.json`。
    pub fn profiles_file(&self) -> PathBuf {
        self.data_dir.join("profiles.json")
    }

    /// 旧版单文件路径 `data_dir/profile.json`（迁移来源）。
    pub fn legacy_file(&self) -> PathBuf {
        self.data_dir.join("profile.json")
    }

    /// 读取全部模板。
    ///
    /// `profiles.json` 缺失时：若旧 `profile.json` 存在则一次性迁移为默认模板并删除
    /// 旧文件；否则返回空列表。`profiles.json` 损坏时记 warning 并回退空列表。
    pub fn load(&self) -> PanelResult<Vec<Profile>> {
        let path = self.profiles_file();
        if path.exists() {
            let text = std::fs::read_to_string(&path)?;
            return match serde_json::from_str(&text) {
                Ok(profiles) => Ok(profiles),
                Err(e) => {
                    tracing::warn!(
                        path = %path.display(),
                        error = %e,
                        "profiles.json unreadable, fall back to empty"
                    );
                    Ok(Vec::new())
                }
            };
        }
        let legacy = self.legacy_file();
        if legacy.exists() {
            let overrides = match std::fs::read_to_string(&legacy)
                .map(|t| serde_json::from_str::<ProfileOverrides>(&t))
            {
                Ok(Ok(ov)) => ov,
                Ok(Err(e)) => {
                    tracing::warn!(
                        path = %legacy.display(),
                        error = %e,
                        "legacy profile.json unreadable, migrate with empty overrides"
                    );
                    ProfileOverrides::default()
                }
                Err(e) => {
                    tracing::warn!(
                        path = %legacy.display(),
                        error = %e,
                        "legacy profile.json unreadable, migrate with empty overrides"
                    );
                    ProfileOverrides::default()
                }
            };
            let profiles = vec![Profile {
                id: Uuid::new_v4(),
                name: "默认".to_string(),
                core_type: CoreType::SingBox,
                yaml_override: overrides.yaml_override,
                js_override: overrides.js_override,
                enabled: true,
            }];
            self.save(&profiles)?;
            if let Err(e) = std::fs::remove_file(&legacy) {
                tracing::warn!(
                    path = %legacy.display(),
                    error = %e,
                    "failed to remove legacy profile.json after migration"
                );
            }
            return Ok(profiles);
        }
        Ok(Vec::new())
    }

    /// 保存全部模板到 `data_dir/profiles.json`。
    pub fn save(&self, profiles: &[Profile]) -> PanelResult<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        let text = serde_json::to_string_pretty(profiles)?;
        std::fs::write(self.profiles_file(), text)?;
        Ok(())
    }

    /// 新增模板：名称与已有模板重复时报错；该核心类型的首个模板自动启用。
    pub fn add(&self, name: &str, core_type: CoreType) -> PanelResult<Profile> {
        let mut profiles = self.load()?;
        if profiles.iter().any(|p| p.name == name) {
            return Err(PanelError::Client(format!(
                "profile with name '{name}' already exists"
            )));
        }
        let profile = Profile {
            id: Uuid::new_v4(),
            name: name.to_string(),
            core_type,
            yaml_override: String::new(),
            js_override: String::new(),
            enabled: !profiles.iter().any(|p| p.core_type == core_type),
        };
        profiles.push(profile.clone());
        self.save(&profiles)?;
        Ok(profile)
    }

    /// 按 id 全量更新模板的可编辑字段（name / yaml_override / js_override）；
    /// `core_type` 与 `enabled` 保持存储值（启用状态经 [`Self::set_enabled`] 切换，
    /// 避免破坏排他不变式）。模板不存在时报错。
    pub fn update(&self, profile: &Profile) -> PanelResult<()> {
        let mut profiles = self.load()?;
        let target = profiles
            .iter_mut()
            .find(|p| p.id == profile.id)
            .ok_or_else(|| PanelError::Client(format!("profile {} not found", profile.id)))?;
        target.name = profile.name.clone();
        target.yaml_override = profile.yaml_override.clone();
        target.js_override = profile.js_override.clone();
        self.save(&profiles)
    }

    /// 按 id 删除模板；不存在时报错。
    pub fn remove(&self, id: Uuid) -> PanelResult<()> {
        let mut profiles = self.load()?;
        let before = profiles.len();
        profiles.retain(|p| p.id != id);
        if profiles.len() == before {
            return Err(PanelError::Client(format!("profile {id} not found")));
        }
        self.save(&profiles)
    }

    /// 设置模板启用状态：启用时同核心类型其他模板自动禁用（排他），不同核心类型
    /// 的启用状态不受影响；禁用直接置 `false`。模板不存在时报错。
    pub fn set_enabled(&self, id: Uuid, enabled: bool) -> PanelResult<()> {
        let mut profiles = self.load()?;
        let target = profiles
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .ok_or_else(|| PanelError::Client(format!("profile {id} not found")))?;
        for p in &mut profiles {
            if p.core_type == target.core_type {
                p.enabled = enabled && p.id == id;
            }
        }
        self.save(&profiles)
    }

    /// 指定核心类型当前启用模板的复写（无启用模板时返回 `None`）。
    pub fn active_for(&self, core_type: CoreType) -> PanelResult<Option<ProfileOverrides>> {
        Ok(self
            .load()?
            .into_iter()
            .find(|p| p.core_type == core_type && p.enabled)
            .map(|p| ProfileOverrides {
                yaml_override: p.yaml_override,
                js_override: p.js_override,
            }))
    }
}

/// sing-box outbound 的叶子协议类型（组与内置类型除外）。
const SINGBOX_LEAF_TYPES: &[&str] = &[
    "vless",
    "vmess",
    "trojan",
    "shadowsocks",
    "shadowsocksr",
    "hysteria",
    "hysteria2",
    "tuic",
    "anytls",
    "wireguard",
    "ssh",
    "http",
    "socks",
];

/// 从 sing-box 订阅配置提取叶子节点（outbounds 中的叶子类型；
/// 排除 selector / urltest / direct / block / dns 等）。tag 去重：重名追加 `-2` / `-3`。
pub fn extract_nodes_singbox(sub: &Value) -> Vec<Value> {
    let leaves: Vec<Value> = sub
        .get("outbounds")
        .and_then(|o| o.as_array())
        .map(|outbounds| {
            outbounds
                .iter()
                .filter(|o| {
                    o.get("type")
                        .and_then(|t| t.as_str())
                        .is_some_and(|t| SINGBOX_LEAF_TYPES.contains(&t))
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    dedup_names(leaves, "tag")
}

/// 从 clash/mihomo 订阅 YAML 提取 `proxies` 节点。name 去重：重名追加 `-2` / `-3`。
pub fn extract_nodes_mihomo(sub_yaml: &str) -> PanelResult<Vec<Value>> {
    let value: Value = serde_yaml::from_str(sub_yaml)
        .map_err(|e| PanelError::Client(format!("invalid clash config in subscription: {e}")))?;
    let proxies = value
        .get("proxies")
        .and_then(|p| p.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(dedup_names(proxies, "name"))
}

/// 按 `key`（sing-box 为 `tag`，mihomo 为 `name`）去重：重名节点追加 `-2` / `-3` …
/// 直至唯一；缺 key 或 key 为空的节点跳过（组引用无法寻址空 tag，跳过更安全）。
fn dedup_names(nodes: Vec<Value>, key: &str) -> Vec<Value> {
    let mut used = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(nodes.len());
    for mut node in nodes {
        let name = match node.get(key).and_then(|n| n.as_str()) {
            Some(name) if !name.is_empty() => name.to_string(),
            _ => continue,
        };
        let mut candidate = name.clone();
        let mut n = 2u32;
        while used.contains(&candidate) {
            candidate = format!("{name}-{n}");
            n += 1;
        }
        used.insert(candidate.clone());
        node[key] = Value::String(candidate);
        out.push(node);
    }
    out
}

/// sing-box 本地模板：log + dns（本地 UDP + 远程 DoH）+ 全部叶子节点 + `proxy`（select，
/// 默认 `auto`）/ `auto`（url-test）组 + `direct` / `block` + 空路由。
///
/// `route.rules` 为空数组（兼容 `compose_singbox_config` 的 MITM 规则前插）；
/// `route.default_domain_resolver` 直接内嵌，保证模板在 sing-box 1.12+ 原生可校验
/// （`dns.servers` 存在时必需）。无叶子节点时 `auto` 组回退到内置 `direct` 保持配置合法。
pub fn singbox_template(nodes: &[Value]) -> Value {
    let tags: Vec<String> = nodes
        .iter()
        .filter_map(|n| n["tag"].as_str().map(String::from))
        .collect();
    let mut auto_outbounds: Vec<Value> = tags.iter().cloned().map(Value::String).collect();
    if auto_outbounds.is_empty() {
        auto_outbounds.push(Value::String("direct".to_string()));
    }
    let mut proxy_outbounds = vec![Value::String("auto".to_string())];
    proxy_outbounds.extend(tags.iter().cloned().map(Value::String));

    let mut cfg = json!({
        "log": { "level": "info" },
        "dns": {
            "servers": [
                { "tag": "local", "type": "udp", "server": "223.5.5.5", "server_port": 53 },
                { "tag": "remote", "type": "https", "server": "8.8.8.8", "server_port": 443 }
            ],
            "strategy": "prefer_ipv4"
        },
        "route": {
            "rules": [],
            "final": "proxy",
            "auto_detect_interface": true,
            "default_domain_resolver": { "server": "local" }
        }
    });
    let mut outbounds = nodes.to_vec();
    outbounds.push(json!({
        "type": "selector",
        "tag": "proxy",
        "outbounds": proxy_outbounds,
        "default": "auto"
    }));
    outbounds.push(json!({
        "type": "urltest",
        "tag": "auto",
        "outbounds": auto_outbounds,
        "url": "https://www.gstatic.com/generate_204",
        "interval": "5m"
    }));
    outbounds.push(json!({ "type": "direct", "tag": "direct" }));
    outbounds.push(json!({ "type": "block", "tag": "block" }));
    cfg["outbounds"] = Value::Array(outbounds);
    cfg
}

/// mihomo 本地模板：dns + 全部 proxies + `proxy`（select，含 `auto`）/ `auto`
/// （url-test，interval 300）组 + `MATCH,proxy` 规则。
///
/// 无叶子节点时 `auto` 组回退到内置 `DIRECT` 保持配置合法。
pub fn mihomo_template(nodes: &[Value]) -> Value {
    let names: Vec<String> = nodes
        .iter()
        .filter_map(|n| n["name"].as_str().map(String::from))
        .collect();
    let mut auto_proxies: Vec<Value> = names.iter().cloned().map(Value::String).collect();
    if auto_proxies.is_empty() {
        auto_proxies.push(Value::String("DIRECT".to_string()));
    }
    let mut proxy_proxies = vec![Value::String("auto".to_string())];
    proxy_proxies.extend(names.iter().cloned().map(Value::String));

    let mut cfg = json!({
        "dns": {
            "enable": true,
            "nameserver": ["223.5.5.5"],
            "fallback": ["dns.google"]
        },
        "proxy-groups": [
            { "name": "proxy", "type": "select", "proxies": proxy_proxies },
            {
                "name": "auto",
                "type": "url-test",
                "proxies": auto_proxies,
                "url": "https://www.gstatic.com/generate_204",
                "interval": 300
            }
        ],
        "rules": ["MATCH,proxy"]
    });
    cfg["proxies"] = Value::Array(nodes.to_vec());
    cfg
}

/// RFC 7386 式深合并：target 与 patch 均为对象时递归合并各键，否则（数组 / 标量）整体替换。
fn merge_deep(target: &mut Value, patch: &Value) {
    if let (Some(t), Some(p)) = (target.as_object_mut(), patch.as_object()) {
        for (key, patch_value) in p {
            match t.get_mut(key) {
                Some(target_value) => merge_deep(target_value, patch_value),
                None => {
                    t.insert(key.clone(), patch_value.clone());
                }
            }
        }
    } else {
        *target = patch.clone();
    }
}

/// YAML 复写：解析 YAML 后按 RFC 7386 深合并进配置。空串 / 空文档 / null 原样返回。
///
/// 顶层必须是 mapping（对象），否则报错。
pub fn apply_yaml_override(config: Value, yaml: &str) -> PanelResult<Value> {
    if yaml.trim().is_empty() {
        return Ok(config);
    }
    let patch: Value = serde_yaml::from_str(yaml)
        .map_err(|e| PanelError::Client(format!("invalid yaml override: {e}")))?;
    if patch.is_null() {
        return Ok(config);
    }
    if !patch.is_object() {
        return Err(PanelError::Client(
            "yaml override must be a YAML mapping".to_string(),
        ));
    }
    let mut merged = config;
    merge_deep(&mut merged, &patch);
    Ok(merged)
}

/// 恒拒绝一切网络请求的 [`HttpExecutor`]：JS 复写环境无网络权限（deny 实现）。
#[derive(Debug)]
pub struct DenyHttpExecutor;

#[async_trait]
impl HttpExecutor for DenyHttpExecutor {
    async fn execute(&self, req: HttpRequestSpec) -> PanelResult<HttpResponseData> {
        Err(PanelError::Client(format!(
            "network access denied in profile JS override: {}",
            req.url
        )))
    }
}

/// JS 复写：同步纯函数模式 `function main(config){...; return config}`。
///
/// 包装源码内嵌配置 JSON（经 `JSON.parse` 还原，避免对象字面量 `__proto__` 陷阱），
/// 结果经 `$done` 回传；`main` 未返回（undefined）时保留原配置。空源码原样返回。
///
/// 宿主：Surge 方言 + [`DenyHttpExecutor`]（无网络）+ 内存存储（无落盘）+
/// [`TracingNotifier`]。限制：2 秒超时、默认 32MB 内存上限。
pub async fn apply_js_override(config: Value, js: &str) -> PanelResult<Value> {
    if js.trim().is_empty() {
        return Ok(config);
    }
    let cfg_json = serde_json::to_string(&config)?;
    let source = format!(
        "let __cfg = JSON.parse({cfg_lit});\n{js}\nlet __r = main(__cfg);\n$done(__r === undefined ? __cfg : __r);",
        cfg_lit = js_string_literal(&cfg_json)
    );
    let host = Arc::new(ScriptHost::new(
        Arc::new(DenyHttpExecutor),
        Arc::new(MemoryPersistentStore::new()),
        Arc::new(TracingNotifier::new()),
    ));
    let mut engine = QuickJsEngine::new(
        host,
        ScriptDialect::Surge,
        ScriptLimits {
            timeout_ms: 2000,
            ..ScriptLimits::default()
        },
        "profile-js".to_string(),
    )?;
    let out = engine
        .run_script(&source, ScriptKind::Generic, None)
        .await?;
    Ok(out.0)
}

/// 把字符串转成安全的 JS 字符串字面量（用于内嵌配置 JSON）。
///
/// `serde_json` 输出已转义控制字符；此处再处理 JSON 自身带出的引号 / 反斜杠，以及
/// QuickJS（ES2019）之前不允许在字符串字面量中出现的 U+2028 / U+2029。
fn js_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// 总装：提取节点 → 本地模板 → YAML 复写 → JS 复写 → 返回核心可用的配置。
///
/// inbounds 与 MITM 链不在此层处理，由 `state` 调用 `compose_*` 注入。
pub async fn build_core_config(
    core_type: CoreType,
    sub_content: &SubContent,
    overrides: &ProfileOverrides,
) -> PanelResult<Value> {
    let config = match (core_type, sub_content) {
        (CoreType::SingBox, SubContent::SingBox(sub)) => {
            singbox_template(&extract_nodes_singbox(sub))
        }
        (CoreType::Mihomo, SubContent::Mihomo(yaml)) => {
            mihomo_template(&extract_nodes_mihomo(yaml)?)
        }
        _ => {
            return Err(PanelError::Client(
                "core type and subscription format mismatch".to_string(),
            ));
        }
    };
    let merged = if overrides.yaml_override.trim().is_empty() {
        config
    } else {
        apply_yaml_override(config, &overrides.yaml_override)?
    };
    if overrides.js_override.trim().is_empty() {
        Ok(merged)
    } else {
        apply_js_override(merged, &overrides.js_override).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_config::{MitmChain, compose_mihomo_config, compose_singbox_config};

    fn sample_singbox_sub() -> Value {
        json!({
            "outbounds": [
                { "type": "vless", "tag": "n1", "server": "example.com", "server_port": 443,
                  "uuid": "12345678-1234-1234-1234-123456789012",
                  "tls": { "enabled": true, "server_name": "example.com" } },
                { "type": "hysteria2", "tag": "n2", "server": "example.org", "server_port": 8443,
                  "password": "pw", "tls": { "enabled": true, "server_name": "example.org" } },
                { "type": "selector", "tag": "proxy", "outbounds": ["n1"] },
                { "type": "urltest", "tag": "auto", "outbounds": ["n1"] },
                { "type": "direct", "tag": "direct" },
                { "type": "block", "tag": "block" }
            ]
        })
    }

    fn sample_mihomo_yaml() -> &'static str {
        r#"
proxies:
  - name: n1
    type: vless
    server: example.com
    port: 443
    uuid: 12345678-1234-1234-1234-123456789012
  - name: n2
    type: hysteria2
    server: example.org
    port: 8443
    password: pw
proxy-groups:
  - name: PROXY
    type: select
    proxies: [n1]
rules:
  - MATCH,DIRECT
"#
    }

    fn mitm_chain() -> MitmChain {
        MitmChain {
            proxy_addr: "127.0.0.1:34567".parse().unwrap(),
            return_port: 17891,
            hostnames: vec!["*.example.com".to_string()],
        }
    }

    /// 真实核心二进制目录：`target/test-cores`（工作区根下）。缺失时相关测试直接跳过。
    fn test_core_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/test-cores")
    }

    fn sing_box_binary() -> Option<PathBuf> {
        let p = test_core_dir().join("sing-box");
        p.is_file().then_some(p)
    }

    fn mihomo_binary() -> Option<PathBuf> {
        let p = test_core_dir().join("mihomo");
        p.is_file().then_some(p)
    }

    /// 本地已下载的 mihomo geoip.metadb（`~/.config/mihomo`），避免 `mihomo -t` 联网下载。
    fn geoip_metadb() -> Option<PathBuf> {
        let p = PathBuf::from(std::env::var("HOME").unwrap_or_default())
            .join(".config/mihomo/geoip.metadb");
        p.is_file().then_some(p)
    }

    // ---------- ① 节点提取（sing-box） ----------

    #[test]
    fn extract_nodes_singbox_keeps_leaves_dedups_tags() {
        let nodes = extract_nodes_singbox(&sample_singbox_sub());
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0]["tag"], "n1");
        assert_eq!(nodes[1]["tag"], "n2");

        // 重名 tag 去重：-2 / -3。
        let sub = json!({
            "outbounds": [
                { "type": "vless", "tag": "dup", "server": "a.com", "server_port": 443 },
                { "type": "vmess", "tag": "dup", "server": "b.com", "server_port": 443 },
                { "type": "trojan", "tag": "dup", "server": "c.com", "server_port": 443 },
                { "type": "selector", "tag": "proxy" }
            ]
        });
        let nodes = extract_nodes_singbox(&sub);
        let tags: Vec<&str> = nodes.iter().map(|n| n["tag"].as_str().unwrap()).collect();
        assert_eq!(tags, vec!["dup", "dup-2", "dup-3"]);

        // 缺 tag 的叶子节点被跳过。
        let sub = json!({
            "outbounds": [
                { "type": "vless", "server": "a.com", "server_port": 443 },
                { "type": "vless", "tag": "ok", "server": "b.com", "server_port": 443 }
            ]
        });
        let nodes = extract_nodes_singbox(&sub);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0]["tag"], "ok");
    }

    // ---------- ② sing-box 模板 ----------

    #[test]
    fn singbox_template_builds_groups_and_route() {
        let nodes = extract_nodes_singbox(&sample_singbox_sub());
        let cfg = singbox_template(&nodes);

        let outbounds = cfg["outbounds"].as_array().unwrap();
        // 叶子节点全量保留。
        assert!(outbounds.iter().any(|o| o["tag"] == "n1"));
        assert!(outbounds.iter().any(|o| o["tag"] == "n2"));

        let proxy = outbounds.iter().find(|o| o["tag"] == "proxy").unwrap();
        assert_eq!(proxy["type"], "selector");
        assert_eq!(proxy["default"], "auto");
        let proxy_out: Vec<&str> = proxy["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(proxy_out, vec!["auto", "n1", "n2"]);

        let auto = outbounds.iter().find(|o| o["tag"] == "auto").unwrap();
        assert_eq!(auto["type"], "urltest");
        assert_eq!(auto["url"], "https://www.gstatic.com/generate_204");
        assert_eq!(auto["interval"], "5m");
        let auto_out: Vec<&str> = auto["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(auto_out, vec!["n1", "n2"]);

        assert!(
            outbounds
                .iter()
                .any(|o| o["tag"] == "direct" && o["type"] == "direct")
        );
        assert!(
            outbounds
                .iter()
                .any(|o| o["tag"] == "block" && o["type"] == "block")
        );

        assert_eq!(cfg["route"]["final"], "proxy");
        assert_eq!(cfg["route"]["rules"], json!([]));
        assert_eq!(cfg["route"]["auto_detect_interface"], true);
        assert_eq!(
            cfg["route"]["default_domain_resolver"],
            json!({ "server": "local" })
        );
        assert_eq!(cfg["log"]["level"], "info");
        assert!(cfg["dns"]["servers"].is_array());
        assert_eq!(cfg["dns"]["strategy"], "prefer_ipv4");
    }

    #[test]
    fn singbox_template_empty_nodes_falls_back_to_direct() {
        let cfg = singbox_template(&[]);
        let auto = cfg["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .find(|o| o["tag"] == "auto")
            .unwrap();
        assert_eq!(auto["outbounds"], json!(["direct"]));
    }

    // ---------- ③ mihomo 提取 + 模板 ----------

    #[test]
    fn extract_nodes_mihomo_reads_proxies_and_dedups_names() {
        let nodes = extract_nodes_mihomo(sample_mihomo_yaml()).unwrap();
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0]["name"], "n1");
        assert_eq!(nodes[1]["name"], "n2");

        let yaml = "proxies:\n  - name: x\n    type: vless\n    server: a.com\n    port: 443\n  - name: x\n    type: vmess\n    server: b.com\n    port: 443\n";
        let nodes = extract_nodes_mihomo(yaml).unwrap();
        let names: Vec<&str> = nodes.iter().map(|n| n["name"].as_str().unwrap()).collect();
        assert_eq!(names, vec!["x", "x-2"]);

        let err = extract_nodes_mihomo("port: [unclosed").unwrap_err();
        assert!(matches!(err, PanelError::Client(_)));
    }

    #[test]
    fn mihomo_template_builds_groups_and_rules() {
        let nodes = extract_nodes_mihomo(sample_mihomo_yaml()).unwrap();
        let cfg = mihomo_template(&nodes);

        assert_eq!(cfg["dns"]["enable"], true);
        assert_eq!(cfg["dns"]["nameserver"], json!(["223.5.5.5"]));

        let proxies = cfg["proxies"].as_array().unwrap();
        assert!(proxies.iter().any(|p| p["name"] == "n1"));
        assert!(proxies.iter().any(|p| p["name"] == "n2"));

        let groups = cfg["proxy-groups"].as_array().unwrap();
        let proxy = groups.iter().find(|g| g["name"] == "proxy").unwrap();
        assert_eq!(proxy["type"], "select");
        let proxy_list: Vec<&str> = proxy["proxies"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(proxy_list, vec!["auto", "n1", "n2"]);

        let auto = groups.iter().find(|g| g["name"] == "auto").unwrap();
        assert_eq!(auto["type"], "url-test");
        assert_eq!(auto["url"], "https://www.gstatic.com/generate_204");
        assert_eq!(auto["interval"], 300);
        let auto_list: Vec<&str> = auto["proxies"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(auto_list, vec!["n1", "n2"]);

        assert_eq!(cfg["rules"], json!(["MATCH,proxy"]));
    }

    #[test]
    fn mihomo_template_empty_nodes_falls_back_to_direct() {
        let cfg = mihomo_template(&[]);
        let auto = cfg["proxy-groups"]
            .as_array()
            .unwrap()
            .iter()
            .find(|g| g["name"] == "auto")
            .unwrap();
        assert_eq!(auto["proxies"], json!(["DIRECT"]));
    }

    // ---------- ④ YAML 深合并复写 ----------

    #[test]
    fn yaml_override_merges_nested_replaces_arrays_adds_keys() {
        let config = json!({
            "route": { "final": "proxy", "rules": [] },
            "dns": { "enable": true, "nameserver": ["1.1.1.1"] },
            "log": { "level": "info" }
        });
        let yaml = r#"
route:
  final: direct
dns:
  nameserver:
    - 223.5.5.5
log:
  level: debug
new-key:
  a: 1
"#;
        let merged = apply_yaml_override(config, yaml).unwrap();
        // 嵌套对象递归合并。
        assert_eq!(merged["route"]["final"], "direct");
        assert_eq!(merged["route"]["rules"], json!([]));
        // 数组整体替换。
        assert_eq!(merged["dns"]["nameserver"], json!(["223.5.5.5"]));
        assert_eq!(merged["dns"]["enable"], true);
        // 标量替换 + 新增键。
        assert_eq!(merged["log"]["level"], "debug");
        assert_eq!(merged["new-key"]["a"], 1);
    }

    #[test]
    fn yaml_override_empty_or_null_keeps_config() {
        let config = json!({ "route": { "final": "proxy" } });
        assert_eq!(apply_yaml_override(config.clone(), "").unwrap(), config);
        assert_eq!(apply_yaml_override(config.clone(), "   ").unwrap(), config);
        assert_eq!(apply_yaml_override(config.clone(), "null").unwrap(), config);
        assert!(
            apply_yaml_override(config, "# only a comment\n")
                .unwrap()
                .as_object()
                .is_some()
        );
    }

    #[test]
    fn yaml_override_rejects_non_mapping_and_bad_yaml() {
        let config = json!({ "route": { "final": "proxy" } });
        assert!(matches!(
            apply_yaml_override(config.clone(), "- a\n- b").unwrap_err(),
            PanelError::Client(_)
        ));
        assert!(matches!(
            apply_yaml_override(config, "route: [unclosed").unwrap_err(),
            PanelError::Client(_)
        ));
    }

    // ---------- ⑤ JS 复写 ----------

    #[tokio::test(flavor = "current_thread")]
    async fn js_override_mutates_config_and_returns() {
        let config = json!({ "route": { "final": "proxy" }, "log": { "level": "info" } });
        let js = r#"function main(c) { c.route.final = "direct"; return c; }"#;
        let out = apply_js_override(config, js).await.unwrap();
        assert_eq!(out["route"]["final"], "direct");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn js_override_without_return_keeps_config() {
        let config = json!({ "route": { "final": "proxy" } });
        let js = "function main(c) { /* intentionally no return */ }";
        let out = apply_js_override(config, js).await.unwrap();
        assert_eq!(out["route"]["final"], "proxy");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn js_override_empty_source_keeps_config() {
        let config = json!({ "route": { "final": "proxy" } });
        assert_eq!(apply_js_override(config.clone(), "").await.unwrap(), config);
        assert_eq!(
            apply_js_override(config, "  ").await.unwrap()["route"]["final"],
            "proxy"
        );
    }

    /// deny 环境：Surge 方言下 `$task` 未注入（undefined，类型级）；`$httpClient` 存在
    /// 但其网络被 [`DenyHttpExecutor`] 拒绝（见 `deny_http_executor_always_denies`）。
    #[tokio::test(flavor = "current_thread")]
    async fn js_override_task_undefined_and_network_denied() {
        let config = json!({ "log": { "level": "info" } });
        let js = r#"
            function main(c) {
                c.taskType = typeof $task;
                c.httpClientType = typeof $httpClient;
                return c;
            }
        "#;
        let out = apply_js_override(config, js).await.unwrap();
        assert_eq!(out["taskType"], "undefined");
        assert_eq!(out["httpClientType"], "object");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn deny_http_executor_always_denies() {
        let err = DenyHttpExecutor
            .execute(HttpRequestSpec {
                url: "http://example.com/".to_string(),
                ..HttpRequestSpec::default()
            })
            .await
            .unwrap_err();
        assert!(matches!(err, PanelError::Client(_)));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn js_override_invalid_script_errors() {
        let config = json!({ "route": { "final": "proxy" } });
        let err = apply_js_override(config, "function main(c) { return c;")
            .await
            .unwrap_err();
        assert!(matches!(err, PanelError::Script(_)));
    }

    // ---------- ⑥ 端到端 ----------

    #[tokio::test(flavor = "current_thread")]
    async fn build_core_config_singbox_end_to_end() {
        let overrides = ProfileOverrides {
            yaml_override: "route:\n  final: direct\n".to_string(),
            js_override: r#"function main(c) { c.log.level = "error"; return c; }"#.to_string(),
        };
        let cfg = build_core_config(
            CoreType::SingBox,
            &SubContent::SingBox(sample_singbox_sub()),
            &overrides,
        )
        .await
        .unwrap();

        // 节点在。
        let outbounds = cfg["outbounds"].as_array().unwrap();
        assert!(outbounds.iter().any(|o| o["tag"] == "n1"));
        assert!(outbounds.iter().any(|o| o["tag"] == "n2"));
        // 分组在。
        let proxy = outbounds.iter().find(|o| o["tag"] == "proxy").unwrap();
        let proxy_list: Vec<&str> = proxy["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(proxy_list.contains(&"n1") && proxy_list.contains(&"n2"));
        // YAML 复写生效。
        assert_eq!(cfg["route"]["final"], "direct");
        // JS 复写（在 YAML 之后）生效。
        assert_eq!(cfg["log"]["level"], "error");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn build_core_config_mihomo_end_to_end() {
        let overrides = ProfileOverrides {
            yaml_override: "rules:\n  - DOMAIN-SUFFIX,example.com,auto\n".to_string(),
            js_override: String::new(),
        };
        let cfg = build_core_config(
            CoreType::Mihomo,
            &SubContent::Mihomo(sample_mihomo_yaml().to_string()),
            &overrides,
        )
        .await
        .unwrap();

        let proxies = cfg["proxies"].as_array().unwrap();
        assert!(proxies.iter().any(|p| p["name"] == "n1"));
        assert!(proxies.iter().any(|p| p["name"] == "n2"));
        let rules = cfg["rules"].as_array().unwrap();
        // YAML 复写整体替换数组（原 MATCH,proxy 被替换为复写内容）。
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0], "DOMAIN-SUFFIX,example.com,auto");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn build_core_config_rejects_format_mismatch() {
        let err = build_core_config(
            CoreType::SingBox,
            &SubContent::Mihomo("proxies: []".to_string()),
            &ProfileOverrides::default(),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, PanelError::Client(_)));
    }

    // ---------- ⑦ 回归：compose_* 注入（模板 route.rules 空数组前插 OK） ----------

    #[tokio::test(flavor = "current_thread")]
    async fn compose_singbox_injects_inbounds_and_mitm_into_profile_output() {
        let cfg = build_core_config(
            CoreType::SingBox,
            &SubContent::SingBox(sample_singbox_sub()),
            &ProfileOverrides::default(),
        )
        .await
        .unwrap();
        assert_eq!(cfg["route"]["rules"], json!([]));

        let composed = compose_singbox_config(&cfg, 17890, Some(mitm_chain())).unwrap();

        // inbounds 注入（主入口 + 回流）。
        let inbounds = composed["inbounds"].as_array().unwrap();
        assert_eq!(inbounds.len(), 2);
        assert_eq!(inbounds[0]["tag"], "main-in");
        assert_eq!(inbounds[1]["tag"], "mitm-return");

        // 模板空 rules 前插 MITM 白名单规则成功，final 保留复写后的 proxy。
        let rules = composed["route"]["rules"].as_array().unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0]["outbound"], "pp-mitm");
        assert_eq!(rules[0]["domain_suffix"], json!(["example.com"]));
        assert_eq!(composed["route"]["final"], "proxy");
        // 分组与节点保留。
        let outbounds = composed["outbounds"].as_array().unwrap();
        assert!(outbounds.iter().any(|o| o["tag"] == "proxy"));
        assert!(outbounds.iter().any(|o| o["tag"] == "n1"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn compose_mihomo_injects_listeners_and_rules_into_profile_output() {
        let cfg = build_core_config(
            CoreType::Mihomo,
            &SubContent::Mihomo(sample_mihomo_yaml().to_string()),
            &ProfileOverrides::default(),
        )
        .await
        .unwrap();
        let yaml = serde_yaml::to_string(&cfg).unwrap();
        let composed = compose_mihomo_config(&yaml, 17890, Some(mitm_chain())).unwrap();

        assert!(composed.get("mixed-port").is_none());
        let listeners = composed["listeners"].as_array().unwrap();
        assert_eq!(listeners.len(), 2);
        let rules = composed["rules"].as_array().unwrap();
        assert_eq!(
            rules[0],
            "AND,((IN-NAME,main-in),(DOMAIN-SUFFIX,example.com)),pp-mitm"
        );
        assert_eq!(rules[1], "MATCH,proxy");
        let proxies = composed["proxies"].as_array().unwrap();
        assert!(proxies.iter().any(|p| p["name"] == "n1"));
        assert!(proxies.iter().any(|p| p["name"] == "pp-mitm"));
    }

    // ---------- 真实核心 check（test-cores 存在时必须验证模板字段兼容） ----------

    #[test]
    fn singbox_template_passes_real_singbox_check() {
        let Some(bin) = sing_box_binary() else {
            return;
        };
        let nodes = extract_nodes_singbox(&sample_singbox_sub());
        let cfg =
            compose_singbox_config(&singbox_template(&nodes), 17890, Some(mitm_chain())).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, serde_json::to_string_pretty(&cfg).unwrap()).unwrap();
        let out = std::process::Command::new(&bin)
            .args(["check", "-c"])
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "sing-box check failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[test]
    fn mihomo_template_passes_real_mihomo_check() {
        let Some(bin) = mihomo_binary() else {
            return;
        };
        let nodes = extract_nodes_mihomo(sample_mihomo_yaml()).unwrap();
        let cfg = compose_mihomo_config(
            &serde_yaml::to_string(&mihomo_template(&nodes)).unwrap(),
            17890,
            Some(mitm_chain()),
        )
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        // 预置 geoip.metadb（存在时）避免 `mihomo -t` 联网下载 geo 数据。
        if let Some(mmdb) = geoip_metadb() {
            std::fs::copy(mmdb, dir.path().join("geoip.metadb")).unwrap();
        }
        let path = dir.path().join("config.yaml");
        std::fs::write(&path, serde_yaml::to_string(&cfg).unwrap()).unwrap();
        let out = std::process::Command::new(&bin)
            .args(["-t", "-f"])
            .arg(&path)
            .arg("-d")
            .arg(dir.path())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "mihomo check failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    // ---------- ProfileStore ----------

    #[test]
    fn profile_store_roundtrip_and_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(dir.path().to_path_buf());

        // 缺失 → 默认。
        assert_eq!(store.load().unwrap(), ProfileOverrides::default());

        let overrides = ProfileOverrides {
            yaml_override: "route:\n  final: direct\n".to_string(),
            js_override: "function main(c) { return c; }".to_string(),
        };
        store.save(&overrides).unwrap();
        assert!(store.profile_file().exists());
        assert_eq!(store.load().unwrap(), overrides);
    }

    #[test]
    fn profile_store_tolerates_corrupted_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(dir.path().to_path_buf());
        std::fs::write(store.profile_file(), "{ not json").unwrap();
        assert_eq!(store.load().unwrap(), ProfileOverrides::default());
    }

    // ---------- ProfileStoreV2（多模板 + 旧版迁移） ----------

    #[test]
    fn profile_store_v2_loads_empty_when_no_files() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStoreV2::new(dir.path().to_path_buf());
        assert_eq!(store.load().unwrap(), Vec::<Profile>::new());
        assert!(!store.profiles_file().exists());
    }

    /// ① 迁移：旧 profile.json → 默认 Profile（SingBox、enabled、复写保留）且旧文件删除。
    #[test]
    fn profile_store_v2_migrates_legacy_profile_json_once() {
        let dir = tempfile::tempdir().unwrap();
        // 预置旧版 profile.json。
        ProfileStore::new(dir.path().to_path_buf())
            .save(&ProfileOverrides {
                yaml_override: "route:\n  final: direct\n".to_string(),
                js_override: "function main(c) { return c; }".to_string(),
            })
            .unwrap();
        let legacy = dir.path().join("profile.json");
        assert!(legacy.exists());

        let store = ProfileStoreV2::new(dir.path().to_path_buf());
        let profiles = store.load().unwrap();

        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].name, "默认");
        assert_eq!(profiles[0].core_type, CoreType::SingBox);
        assert!(profiles[0].enabled);
        assert_eq!(profiles[0].yaml_override, "route:\n  final: direct\n");
        assert_eq!(profiles[0].js_override, "function main(c) { return c; }");

        // 一次性迁移：旧文件已删除，二次 load 不再迁移、id 保持。
        assert!(!legacy.exists());
        let again = store.load().unwrap();
        assert_eq!(again.len(), 1);
        assert_eq!(again[0].id, profiles[0].id);
    }

    #[test]
    fn profile_store_v2_migrates_corrupted_legacy_with_empty_overrides() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("profile.json"), "{ not json").unwrap();
        let store = ProfileStoreV2::new(dir.path().to_path_buf());
        let profiles = store.load().unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].name, "默认");
        assert_eq!(profiles[0].yaml_override, "");
        assert_eq!(profiles[0].js_override, "");
        assert!(!dir.path().join("profile.json").exists());
    }

    /// ② add：首个该 core_type 自动 enabled；第二个同 core_type 不自动；重名报错。
    #[test]
    fn profile_store_v2_add_first_auto_enabled_second_not() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStoreV2::new(dir.path().to_path_buf());

        let a = store.add("A", CoreType::SingBox).unwrap();
        assert!(a.enabled, "首个该 core_type 的模板应自动启用");
        assert!(!a.id.is_nil());

        let b = store.add("B", CoreType::SingBox).unwrap();
        assert!(!b.enabled, "第二个同 core_type 模板不应自动启用");

        // 不同 core_type 的首个模板自动启用，且不影响 SingBox 的启用状态。
        let c = store.add("C", CoreType::Mihomo).unwrap();
        assert!(c.enabled);

        // 重名报错（跨核心也报错）。
        let err = store.add("A", CoreType::Mihomo).unwrap_err();
        assert!(matches!(err, PanelError::Client(_)));

        // 磁盘状态与内存一致：SingBox 恰有一条 enabled。
        let loaded = store.load().unwrap();
        assert_eq!(loaded.len(), 3);
        assert_eq!(
            loaded
                .iter()
                .filter(|p| p.core_type == CoreType::SingBox && p.enabled)
                .count(),
            1
        );
    }

    /// ③ set_enabled 排他：启用 B 后 A 自动禁用（同 core_type），跨 core_type 不受影响。
    #[test]
    fn profile_store_v2_set_enabled_exclusive() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStoreV2::new(dir.path().to_path_buf());
        let a = store.add("A", CoreType::SingBox).unwrap();
        let b = store.add("B", CoreType::SingBox).unwrap();
        let m = store.add("M", CoreType::Mihomo).unwrap();
        assert!(a.enabled);
        assert!(!b.enabled);
        assert!(m.enabled);

        // 启用 B → A 自动禁用、M（不同 core_type）保持启用。
        store.set_enabled(b.id, true).unwrap();
        let loaded = store.load().unwrap();
        let by_id = |id: Uuid| loaded.iter().find(|p| p.id == id).unwrap();
        assert!(!by_id(a.id).enabled);
        assert!(by_id(b.id).enabled);
        assert!(by_id(m.id).enabled);

        // active_for 返回 B 的复写。
        let active = store.active_for(CoreType::SingBox).unwrap();
        assert!(active.is_some());
        assert_eq!(active.as_ref().unwrap().yaml_override, "");

        // 禁用 B → SingBox 无启用模板。
        store.set_enabled(b.id, false).unwrap();
        assert!(store.active_for(CoreType::SingBox).unwrap().is_none());
        assert!(store.active_for(CoreType::Mihomo).unwrap().is_some());

        // 不存在 id 报错。
        let err = store.set_enabled(Uuid::new_v4(), true).unwrap_err();
        assert!(matches!(err, PanelError::Client(_)));
    }

    /// ④ update/remove 语义：update 按 id 全量更新 name/yaml/js（enabled 保持），
    /// remove 按 id 删除；均对不存在 id 报错。
    #[test]
    fn profile_store_v2_update_and_remove() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStoreV2::new(dir.path().to_path_buf());
        let mut p = store.add("A", CoreType::SingBox).unwrap();

        p.name = "A-renamed".to_string();
        p.yaml_override = "route:\n  final: direct\n".to_string();
        p.js_override = "function main(c) { return c; }".to_string();
        p.enabled = false; // 传入无效，应保持存储值。
        store.update(&p).unwrap();

        let loaded = store.load().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "A-renamed");
        assert_eq!(loaded[0].yaml_override, "route:\n  final: direct\n");
        assert_eq!(loaded[0].js_override, "function main(c) { return c; }");
        assert!(loaded[0].enabled, "update 不改启用状态");
        assert_eq!(loaded[0].core_type, CoreType::SingBox);

        // 不存在 id 的 update 报错。
        let ghost = Profile {
            id: Uuid::new_v4(),
            ..p.clone()
        };
        assert!(matches!(
            store.update(&ghost).unwrap_err(),
            PanelError::Client(_)
        ));

        // remove：按 id 删除后为空。
        store.remove(p.id).unwrap();
        assert_eq!(store.load().unwrap().len(), 0);

        // 不存在 id 的 remove 报错。
        assert!(matches!(
            store.remove(Uuid::new_v4()).unwrap_err(),
            PanelError::Client(_)
        ));
    }

    /// active_for 返回启用模板的复写字段（空串保留）。
    #[test]
    fn profile_store_v2_active_for_returns_overrides_of_enabled_profile() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStoreV2::new(dir.path().to_path_buf());
        let a = store.add("A", CoreType::SingBox).unwrap();
        store
            .update(&Profile {
                yaml_override: "route:\n  final: direct\n".to_string(),
                js_override: "function main(c) { return c; }".to_string(),
                ..a.clone()
            })
            .unwrap();
        let active = store.active_for(CoreType::SingBox).unwrap().unwrap();
        assert_eq!(active.yaml_override, "route:\n  final: direct\n");
        assert_eq!(active.js_override, "function main(c) { return c; }");
    }
}
