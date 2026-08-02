//! Profile 层：订阅仅取节点，本地模板生成规则，支持 YAML 深合并复写 + JS 复写。
//!
//! 与 Clash Verge 的 Merge + Script 对齐：
//! - 订阅内容只用于提取代理节点（sing-box `outbounds` 叶子 / mihomo `proxies`），
//!   客户端实际运行配置由本地模板生成，避免订阅自带的分组 / 规则 / 路由覆盖本地。
//! - YAML 复写（[`apply_yaml_override`]）按 RFC 7386 深合并：对象递归合并，数组与
//!   标量整体替换。
//! - JS 复写（[`apply_js_override`]）为同步纯函数模式 `function main(config){...; return config}`，
//!   经 pp-script [`ScriptWorker`]（专有线程 + current_thread 运行时，`Send` future）
//!   驱动，宿主以 [`DenyHttpExecutor`] 拒绝一切网络、以内存存储承担持久化
//!   （无落盘），即"无网络 / 无存储权限"。
//!
//! 总装入口 [`build_core_config_v2`]（远程 + 本地叠加；旧签名 [`build_core_config`]
//! 兼容保留）：提取节点 → 本地模板 → 远程 YAML → 本地 YAML → 远程 JS → 本地 JS。
//! 远程复写 URL 经 [`resolve_remote_overrides`] 拉取并缓存回退。
//! inbounds 与 MITM 链不在本层，仍由 [`crate::state`] 通过
//! [`crate::core_config`] 的 `compose_*` 注入。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use pp_common::{CoreType, PanelError, PanelResult};
use pp_script::{
    HttpExecutor, HttpRequestSpec, HttpResponseData, MemoryPersistentStore, ScriptDialect,
    ScriptHost, ScriptKind, ScriptLimits, ScriptWorker,
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
/// 作为旧版单文件存储 [`ProfileStore`] 的载荷（仅兼容遗留调用方）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileOverrides {
    /// YAML 深合并复写（RFC 7386 式；空串 = 未启用）。
    pub yaml_override: String,
    /// JS 复写（同步纯函数 `function main(config){...; return config}`；空串 = 未启用）。
    pub js_override: String,
}

/// 远程 + 本地叠加后的有效复写（远程为基底、本地覆盖）。
///
/// 由 [`resolve_remote_overrides`] 产出：`remote_*` 为远程 URL 拉取/缓存回退的
/// 内容，`local_*` 为 Profile 本地复写。由 [`build_core_config_v2`] 消费：
/// YAML 阶段先应用远程再应用本地（深合并天然满足本地覆盖）；JS 阶段远程 `main`
/// 先执行、本地 `main` 后执行（链式，本地可见远程结果）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EffectiveOverrides {
    /// 远程 YAML 复写内容（空串 = 无）。
    pub remote_yaml: String,
    /// 本地 YAML 复写内容（空串 = 无）。
    pub local_yaml: String,
    /// 远程 JS 复写源码（空串 = 无）。
    pub remote_js: String,
    /// 本地 JS 复写源码（空串 = 无）。
    pub local_js: String,
}

/// 一个 Profile 模板（纯关联制）：同一核心类型（[`CoreType`]）可维护多个，
/// 运行时使用的覆写 = 当前选中订阅关联的模板，模板本身不持有启用状态。
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
    /// 远程 YAML 复写 URL（http/https；启动时拉取，失败回退缓存；`None` = 未配置）。
    #[serde(default)]
    pub yaml_url: Option<String>,
    /// 远程 JS 复写 URL（http/https；启动时拉取，失败回退缓存；`None` = 未配置）。
    #[serde(default)]
    pub js_url: Option<String>,
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
/// 纯关联制：模板不持有启用状态，运行时使用的覆写 = 当前选中订阅关联的模板
/// （见 `crate::state` 启动流程与订阅的 `profile_id`）。
///
/// 旧版单文件 `data_dir/profile.json`（[`ProfileStore`]）在首次 [`ProfileStoreV2::load`]
/// 时一次性迁移为 `Profile{name:"默认", core_type: SingBox}`（复写内容原样保留）
/// 并删除旧文件；`profiles.json` 已存在时不做迁移。
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
                yaml_url: None,
                js_url: None,
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

    /// 新增模板：名称与已有模板重复时报错。
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
            yaml_url: None,
            js_url: None,
        };
        profiles.push(profile.clone());
        self.save(&profiles)?;
        Ok(profile)
    }

    /// 按 id 全量更新模板的可编辑字段（name / yaml_override / js_override /
    /// yaml_url / js_url）；`core_type` 保持存储值。模板不存在时报错。
    pub fn update(&self, profile: &Profile) -> PanelResult<()> {
        let mut profiles = self.load()?;
        let target = profiles
            .iter_mut()
            .find(|p| p.id == profile.id)
            .ok_or_else(|| PanelError::Client(format!("profile {} not found", profile.id)))?;
        target.name = profile.name.clone();
        target.yaml_override = profile.yaml_override.clone();
        target.js_override = profile.js_override.clone();
        target.yaml_url = profile.yaml_url.clone();
        target.js_url = profile.js_url.clone();
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
///
/// 执行经 [`ScriptWorker`]（专有线程 + current_thread 运行时）驱动，返回 `Send`
/// future；按用创建 worker（一次线程 spawn），job 完成后随 `tx` 释放线程自然退出。
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
    let worker = ScriptWorker::new(
        host,
        ScriptLimits {
            timeout_ms: 2000,
            ..ScriptLimits::default()
        },
    );
    let out = worker
        .run_script(
            &source,
            ScriptKind::Generic,
            None,
            None,
            ScriptDialect::Surge,
            "profile-js",
        )
        .await?;
    Ok(out.0)
}

/// 解析 Profile 的远程复写 URL：启动时拉取（no_proxy、30 秒超时、默认 UA
/// `clash.meta`），成功写缓存 `profile_cache/<profile_id>.{yaml,js}`；失败回退
/// 缓存；缓存也没有 → 记 warning 跳过该远程复写（不阻塞启动）。
///
/// 返回叠加后的有效复写（远程为基底、本地覆盖）与警告列表；`ProfileOverrides`
/// 不在此层产出，YAML/JS 的「远程 + 本地」合并交由 [`build_core_config_v2`]。
pub async fn resolve_remote_overrides(
    store_cache_dir: &Path,
    profile: &Profile,
) -> (EffectiveOverrides, Vec<String>) {
    let mut warnings = Vec::new();
    let key = profile.id.to_string();
    let remote_yaml = match profile.yaml_url.as_deref() {
        Some(url) if !url.trim().is_empty() => {
            fetch_remote_override(store_cache_dir, &key, url, "yaml", &mut warnings).await
        }
        _ => String::new(),
    };
    let remote_js = match profile.js_url.as_deref() {
        Some(url) if !url.trim().is_empty() => {
            fetch_remote_override(store_cache_dir, &key, url, "js", &mut warnings).await
        }
        _ => String::new(),
    };
    (
        EffectiveOverrides {
            remote_yaml,
            local_yaml: profile.yaml_override.clone(),
            remote_js,
            local_js: profile.js_override.clone(),
        },
        warnings,
    )
}

/// 拉取单个远程复写：成功写缓存；失败回退缓存；均失败记 warning 返回空串。
async fn fetch_remote_override(
    cache_dir: &Path,
    key: &str,
    url: &str,
    ext: &str,
    warnings: &mut Vec<String>,
) -> String {
    match fetch_remote_text(url).await {
        Ok(text) => {
            if let Err(e) = write_override_cache(cache_dir, key, ext, &text) {
                warnings.push(format!("profile remote {ext} cache write failed: {e}"));
            }
            text
        }
        Err(e) => match read_override_cache(cache_dir, key, ext) {
            Ok(Some(text)) => {
                warnings.push(format!(
                    "profile remote {ext} fetch failed, fall back to cached: {e}"
                ));
                text
            }
            Ok(None) => {
                warnings.push(format!(
                    "profile remote {ext} fetch failed and no cached copy, skipped: {e}"
                ));
                String::new()
            }
            Err(read_err) => {
                warnings.push(format!(
                    "profile remote {ext} fetch failed and cached copy unreadable, \
                     skipped: {e}; cache: {read_err}"
                ));
                String::new()
            }
        },
    }
}

/// GET 拉取远程复写文本：no_proxy、30 秒超时、默认 UA `clash.meta`；非 2xx 视为失败。
async fn fetch_remote_text(url: &str) -> PanelResult<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .no_proxy()
        .user_agent("clash.meta")
        .build()
        .map_err(|e| PanelError::Client(format!("failed to build http client: {e}")))?;
    let resp =
        client.get(url).send().await.map_err(|e| {
            PanelError::Client(format!("remote override fetch failed ({url}): {e}"))
        })?;
    let status = resp.status();
    if !status.is_success() {
        return Err(PanelError::Client(format!(
            "remote override fetch returned HTTP {status} ({url})"
        )));
    }
    resp.text().await.map_err(|e| {
        PanelError::Client(format!("failed to read remote override body ({url}): {e}"))
    })
}

/// 远程复写缓存路径：`<cache_dir>/<key>.<ext>`（key 为 profile id）。
fn override_cache_path(cache_dir: &Path, key: &str, ext: &str) -> PathBuf {
    cache_dir.join(format!("{key}.{ext}"))
}

/// 写远程复写缓存（成功静默；失败返回 Err 由调用方记 warning）。
fn write_override_cache(cache_dir: &Path, key: &str, ext: &str, content: &str) -> PanelResult<()> {
    std::fs::create_dir_all(cache_dir)?;
    std::fs::write(override_cache_path(cache_dir, key, ext), content)?;
    Ok(())
}

/// 读远程复写缓存：缺失返回 `None`。
fn read_override_cache(cache_dir: &Path, key: &str, ext: &str) -> PanelResult<Option<String>> {
    let path = override_cache_path(cache_dir, key, ext);
    if !path.exists() {
        return Ok(None);
    }
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(Some(text)),
        Err(e) => Err(PanelError::Client(format!(
            "read profile override cache failed: {e}"
        ))),
    }
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

/// 组合远程 + 本地两段 JS 复写为链式源码：远程 `main` 先执行、本地 `main` 后执行
/// （本地可见远程结果）。两段代码各自定义 `main` 会重名，用 IIFE 各自捕获后构建
/// 一个顶层链式 `main`（与 [`apply_js_override`] 包装器末尾的 `main(__cfg)` 调用
/// 对齐）。返回空串当且仅当两段均空（调用方跳过 JS 阶段）。
fn compose_js_chain(remote_js: &str, local_js: &str) -> String {
    let remote = remote_js.trim();
    let local = local_js.trim();
    if remote.is_empty() && local.is_empty() {
        return String::new();
    }
    let mut src = String::new();
    if !remote.is_empty() {
        src.push_str("let __r_main = (function() {\n");
        src.push_str(remote_js);
        src.push_str("\nreturn main;\n})();\n");
    }
    if !local.is_empty() {
        src.push_str("let __l_main = (function() {\n");
        src.push_str(local_js);
        src.push_str("\nreturn main;\n})();\n");
    }
    src.push_str("function main(__cfg) {\n");
    match (remote.is_empty(), local.is_empty()) {
        (false, false) => src.push_str("  return __l_main(__r_main(__cfg));\n"),
        (false, true) => src.push_str("  return __r_main(__cfg);\n"),
        (true, false) => src.push_str("  return __l_main(__cfg);\n"),
        (true, true) => {}
    }
    src.push_str("}\n");
    src
}

/// 总装（v2，支持远程复写叠加）：提取节点 → 本地模板 → 远程 YAML → 本地 YAML →
/// 远程 JS → 本地 JS → 返回核心可用的配置。
///
/// 叠加语义：远程为基底、本地覆盖——YAML 阶段先应用远程再应用本地（两次深合并
/// 天然满足本地覆盖）；JS 阶段远程 `main` 先执行、本地 `main` 后执行（链式，
/// 本地可见远程结果）。inbounds 与 MITM 链不在此层处理，由 `state` 调用
/// `compose_*` 注入。
pub async fn build_core_config_v2(
    core_type: CoreType,
    sub_content: &SubContent,
    effective: &EffectiveOverrides,
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
    // YAML 阶段：远程为基底、本地叠加（两次应用天然满足本地覆盖）。
    let merged = apply_yaml_override(config, &effective.remote_yaml)?;
    let merged = apply_yaml_override(merged, &effective.local_yaml)?;
    // JS 阶段：远程 main 先执行、本地 main 后执行（IIFE 隔离重名，链式调用）。
    let js = compose_js_chain(&effective.remote_js, &effective.local_js);
    if js.is_empty() {
        Ok(merged)
    } else {
        apply_js_override(merged, &js).await
    }
}

/// 总装（旧签名兼容）：提取节点 → 本地模板 → YAML 复写 → JS 复写 → 返回核心可用配置。
///
/// 仅本地复写（无远程 URL）；远程复写叠加场景请使用 [`build_core_config_v2`]。
/// inbounds 与 MITM 链不在此层处理，由 `state` 调用 `compose_*` 注入。
pub async fn build_core_config(
    core_type: CoreType,
    sub_content: &SubContent,
    overrides: &ProfileOverrides,
) -> PanelResult<Value> {
    build_core_config_v2(
        core_type,
        sub_content,
        &EffectiveOverrides {
            remote_yaml: String::new(),
            local_yaml: overrides.yaml_override.clone(),
            remote_js: String::new(),
            local_js: overrides.js_override.clone(),
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_config::{MitmChain, compose_mihomo_config, compose_singbox_config};

    use std::net::SocketAddr;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use axum::http::StatusCode;

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

    // ---------- ⑥ Send 编译期断言 ----------

    /// 编译期断言：`build_core_config` 的 future 为 `Send`（`apply_js_override`
    /// 经 [`ScriptWorker`] 驱动后不再含 rquickjs 非 `Send` 结构，可跨线程 await）。
    #[test]
    fn build_core_config_future_is_send() {
        fn assert_send<T: Send>(_: &T) {}
        let sub = SubContent::SingBox(sample_singbox_sub());
        let overrides = ProfileOverrides::default();
        let fut = build_core_config(CoreType::SingBox, &sub, &overrides);
        assert_send(&fut);
    }

    // ---------- ⑦ 端到端 ----------

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

    // ---------- ⑧ 回归：compose_* 注入（模板 route.rules 空数组前插 OK） ----------

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

    /// ② add：新模板不携带启用状态（纯关联制）；重名报错（跨核心也报错）。
    #[test]
    fn profile_store_v2_add_creates_profiles_without_enabled() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStoreV2::new(dir.path().to_path_buf());

        let a = store.add("A", CoreType::SingBox).unwrap();
        assert!(!a.id.is_nil());

        let b = store.add("B", CoreType::SingBox).unwrap();
        assert_ne!(a.id, b.id);

        let c = store.add("C", CoreType::Mihomo).unwrap();
        assert_ne!(a.id, c.id);

        // 重名报错（跨核心也报错）。
        let err = store.add("A", CoreType::Mihomo).unwrap_err();
        assert!(matches!(err, PanelError::Client(_)));

        // 磁盘状态与内存一致。
        let loaded = store.load().unwrap();
        assert_eq!(loaded.len(), 3);
    }

    /// ④ update/remove 语义：update 按 id 全量更新 name/yaml/js，remove 按 id 删除；
    /// 均对不存在 id 报错。
    #[test]
    fn profile_store_v2_update_and_remove() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStoreV2::new(dir.path().to_path_buf());
        let mut p = store.add("A", CoreType::SingBox).unwrap();

        p.name = "A-renamed".to_string();
        p.yaml_override = "route:\n  final: direct\n".to_string();
        p.js_override = "function main(c) { return c; }".to_string();
        p.yaml_url = Some("https://example.com/r.yaml".to_string());
        p.js_url = Some("https://example.com/r.js".to_string());
        store.update(&p).unwrap();

        let loaded = store.load().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "A-renamed");
        assert_eq!(loaded[0].yaml_override, "route:\n  final: direct\n");
        assert_eq!(loaded[0].js_override, "function main(c) { return c; }");
        assert_eq!(
            loaded[0].yaml_url.as_deref(),
            Some("https://example.com/r.yaml")
        );
        assert_eq!(
            loaded[0].js_url.as_deref(),
            Some("https://example.com/r.js")
        );
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

    // ---------- ⑩ 远程复写 URL：resolve_remote_overrides + 叠加 ----------

    /// 便捷构造 Profile（默认无远程 URL、空本地复写）。
    fn remote_test_profile(yaml_url: Option<String>, js_url: Option<String>) -> Profile {
        Profile {
            id: Uuid::new_v4(),
            name: "远程".to_string(),
            core_type: CoreType::SingBox,
            yaml_override: String::new(),
            js_override: String::new(),
            yaml_url,
            js_url,
        }
    }

    /// 启动本地服务：首个请求返回 `first_body`，此后一律 500（验证缓存回退）。
    async fn spawn_toggle_server(first_body: &'static str) -> SocketAddr {
        let hits = Arc::new(AtomicUsize::new(0));
        let app_hits = Arc::clone(&hits);
        let app = axum::Router::new().fallback(move |_req: axum::extract::Request| {
            let app_hits = Arc::clone(&app_hits);
            async move {
                if app_hits.fetch_add(1, Ordering::SeqCst) == 0 {
                    (StatusCode::OK, first_body)
                } else {
                    (StatusCode::INTERNAL_SERVER_ERROR, "oops")
                }
            }
        });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        addr
    }

    /// 启动本地服务：所有请求返回 `body`。
    async fn spawn_ok_server(body: &'static str) -> SocketAddr {
        let app = axum::Router::new().fallback(move || async move { (StatusCode::OK, body) });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        addr
    }

    /// 启动本地服务：所有请求一律 500（验证「失败且无缓存」路径）。
    async fn spawn_500_server() -> SocketAddr {
        let app = axum::Router::new()
            .fallback(move || async move { (StatusCode::INTERNAL_SERVER_ERROR, "oops") });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        addr
    }

    /// ① 远程 YAML + 本地 YAML 叠加：远程 a=1、b=1；本地 b=2 → 最终 a=1、b=2。
    #[tokio::test(flavor = "current_thread")]
    async fn v2_yaml_remote_then_local_overlay() {
        let sub = SubContent::SingBox(sample_singbox_sub());
        let effective = EffectiveOverrides {
            remote_yaml: "a: 1\nb: 1\n".to_string(),
            local_yaml: "b: 2\n".to_string(),
            ..EffectiveOverrides::default()
        };
        let cfg = build_core_config_v2(CoreType::SingBox, &sub, &effective)
            .await
            .unwrap();
        assert_eq!(cfg["a"], 1, "远程新增键应保留");
        assert_eq!(cfg["b"], 2, "本地应覆盖远程的 b");
    }

    /// ② 远程 JS + 本地 JS 链式：远程 main 改 x=1，本地 main 改 y=x+1 → 本地可见远程结果。
    #[tokio::test(flavor = "current_thread")]
    async fn v2_js_remote_then_local_chain() {
        let sub = SubContent::SingBox(sample_singbox_sub());
        let effective = EffectiveOverrides {
            remote_js: "function main(c) { c.x = 1; return c; }".to_string(),
            local_js: "function main(c) { c.y = c.x + 1; return c; }".to_string(),
            ..EffectiveOverrides::default()
        };
        let cfg = build_core_config_v2(CoreType::SingBox, &sub, &effective)
            .await
            .unwrap();
        assert_eq!(cfg["x"], 1, "远程 main 应生效");
        assert_eq!(cfg["y"], 2, "本地 main 应看到远程结果（y = x + 1）");
    }

    /// ③ 拉取失败回退缓存：先成功一次写缓存（yaml/js），再 500 → 用缓存内容。
    #[tokio::test(flavor = "current_thread")]
    async fn resolve_remote_overrides_fetches_writes_cache_and_falls_back() {
        let yaml_body = "route:\n  final: direct\n";
        let js_body = "function main(c) { c.log.level = \"error\"; return c; }";
        let toggle_addr = spawn_toggle_server(yaml_body).await;
        let ok_addr = spawn_ok_server(js_body).await;
        let dir = tempfile::tempdir().unwrap();
        let profile = remote_test_profile(
            Some(format!("http://{toggle_addr}/yaml")),
            Some(format!("http://{ok_addr}/js")),
        );
        let cache = dir.path();

        // 第一次：yaml/js 均拉取成功并写缓存，无警告。
        let (effective, warnings) = resolve_remote_overrides(cache, &profile).await;
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        assert_eq!(effective.remote_yaml, yaml_body);
        assert_eq!(effective.remote_js, js_body);
        let yaml_cache = cache.join(format!("{}.yaml", profile.id));
        let js_cache = cache.join(format!("{}.js", profile.id));
        assert_eq!(std::fs::read_to_string(&yaml_cache).unwrap(), yaml_body);
        assert_eq!(std::fs::read_to_string(&js_cache).unwrap(), js_body);

        // 第二次：yaml 拉取失败（500）回退缓存；js 仍成功。
        let (effective, warnings) = resolve_remote_overrides(cache, &profile).await;
        assert_eq!(effective.remote_yaml, yaml_body, "yaml 应回退到缓存");
        assert_eq!(effective.remote_js, js_body, "js 仍成功拉取");
        assert!(
            warnings
                .iter()
                .any(|w| w.contains("yaml") && w.contains("fall back to cached")),
            "warnings: {warnings:?}"
        );

        // 叠加全链路：远程 YAML 基底 + 本地覆盖 + 远程 JS。
        let effective = EffectiveOverrides {
            local_yaml: "route:\n  final: block\n".to_string(),
            ..effective
        };
        let sub = SubContent::SingBox(sample_singbox_sub());
        let cfg = build_core_config_v2(CoreType::SingBox, &sub, &effective)
            .await
            .unwrap();
        assert_eq!(cfg["route"]["final"], "block", "本地 YAML 应覆盖远程");
        assert_eq!(cfg["log"]["level"], "error", "远程 JS 应生效");
    }

    /// ④ 拉取失败且无缓存 → warning 跳过该远程复写（remote 为空串），不报错。
    #[tokio::test(flavor = "current_thread")]
    async fn resolve_remote_overrides_fetch_failure_without_cache_warns_and_skips() {
        let addr = spawn_500_server().await;
        let dir = tempfile::tempdir().unwrap();
        let profile = remote_test_profile(Some(format!("http://{addr}/yaml")), None);

        let (effective, warnings) = resolve_remote_overrides(dir.path(), &profile).await;
        assert_eq!(effective.remote_yaml, "", "无缓存应跳过远程复写");
        assert_eq!(effective.local_yaml, "");
        assert!(
            warnings.iter().any(|w| w.contains("no cached copy")),
            "warnings: {warnings:?}"
        );
        assert!(!dir.path().join(format!("{}.yaml", profile.id)).exists());
    }

    /// ⑤ 纯本地（无 URL）回归：resolve 产出本地复写，v2 与旧签名 build_core_config 一致。
    #[tokio::test(flavor = "current_thread")]
    async fn resolve_remote_overrides_pure_local_regression() {
        let dir = tempfile::tempdir().unwrap();
        let profile = Profile {
            yaml_override: "route:\n  final: direct\n".to_string(),
            js_override: "function main(c) { c.log.level = \"error\"; return c; }".to_string(),
            ..remote_test_profile(None, None)
        };

        let (effective, warnings) = resolve_remote_overrides(dir.path(), &profile).await;
        assert!(warnings.is_empty(), "warnings: {warnings:?}");
        assert_eq!(effective.remote_yaml, "");
        assert_eq!(effective.remote_js, "");
        assert_eq!(effective.local_yaml, profile.yaml_override);
        assert_eq!(effective.local_js, profile.js_override);

        let sub = SubContent::SingBox(sample_singbox_sub());
        let legacy = build_core_config(
            CoreType::SingBox,
            &sub,
            &ProfileOverrides {
                yaml_override: profile.yaml_override.clone(),
                js_override: profile.js_override.clone(),
            },
        )
        .await
        .unwrap();
        let v2 = build_core_config_v2(CoreType::SingBox, &sub, &effective)
            .await
            .unwrap();
        assert_eq!(legacy, v2, "v2 纯本地应与旧签名行为一致");
        assert_eq!(v2["route"]["final"], "direct");
        assert_eq!(v2["log"]["level"], "error");
    }

    /// 编译期断言：`build_core_config_v2` / `resolve_remote_overrides` 的 future 为 `Send`。
    #[test]
    fn remote_overrides_futures_are_send() {
        fn assert_send<T: Send>(_: &T) {}
        let sub = SubContent::SingBox(sample_singbox_sub());
        let effective = EffectiveOverrides::default();
        let fut = build_core_config_v2(CoreType::SingBox, &sub, &effective);
        assert_send(&fut);
        let profile = remote_test_profile(None, None);
        let fut = resolve_remote_overrides(Path::new("/tmp"), &profile);
        assert_send(&fut);
    }
}
