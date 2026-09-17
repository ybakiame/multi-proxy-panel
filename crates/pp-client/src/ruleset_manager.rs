//! 内置 CN 分流规则集的本地化管理（下载 / 镜像回退 / 降级启动）。
//!
//! # 背景
//!
//! sing-box 启动阶段**同步下载** `type: remote` 规则集，URL 不可达即核心启动失败（已对
//! 1.14 实测）。早期配置把 5 个 MetaCubeX 规则集写死为 jsDelivr 远程地址：在 jsDelivr
//! 不可达、或直连网络完全不可用（如本 issue 的 `no available network interface`）的地
//! 区 / 网络状态下，首次启动（无本地文件）直接无法起核心。
//!
//! # 方案
//!
//! 规则集改为 **App 侧下载 + `type: local` 本地文件**：
//!
//! 1. 启动核心前，[`ensure_builtin_rule_sets`] 逐个检查 `data_dir/rulesets/builtin/<tag>.srs`
//!    ——已存在（非空）直接使用；缺失则按镜像顺序回退下载（每个镜像 10s 超时，全部
//!    失败且无旧文件时该 tag 记为缺失）：
//!    ① jsDelivr 镜像（CN 友好 CDN）→ ② GitHub raw 原始地址 → ③ 用户配置的
//!    GitHub 代理前缀 + GitHub raw（`github_proxy_prefix` 设置，与核心下载器同一套
//!    [`crate::apply_github_proxy_prefix`] 语义）；
//! 2. [`materialize_rule_sets`] 对合成完成的配置做后处理：可用的 CN 规则集条目改写为
//!    `type: local` 指向本地文件；**缺失的规则集条目连同引用它们的路由 / DNS 规则一并
//!    移除（降级启动）**——CN 分流退化为 `route.final` 兜底（流量走代理，可用但不精
//!    准），而不是整核拒绝启动；
//! 3. 缺失 tag 经 [`MaterializeReport::missing_tags`] 上报到运行状态，命令层启动后后台
//!    重试下载，补齐后自动重载核心恢复完整分流（见 `pp-client-tauri` 的启动重试任务）。
//!
//! 下载统一 `.no_proxy()` 直连（与此前核心侧 `rule-set-direct` http_client 的语义一致：
//! 规则集是核心启动的前置条件，不能依赖代理路径）。
//!
//! 镜像回退也解决了「写死 jsDelivr」问题：GitHub 原始地址经用户 GitHub 代理前缀可达
//! 时同样可完成下载。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value;

/// 单个镜像源（按尝试顺序）。
const MIRROR_JSDELIVR: &str = "jsdelivr";
const MIRROR_GITHUB: &str = "github";

/// Built-in rule set directory (tag → mirror URL template).
///
/// Same source family as [`crate::core_config::CN_RULE_SETS`]; since 2026-09 only the private
/// domain / private IP tags remain (country lists are user opt-in from the market). URLs come
/// in two flavours — jsDelivr (`@sing` ref) and GitHub raw (`sing` branch) — composed as
/// `geo/geosite|geoip/<name>.srs`.
const BUILTIN_RULE_SETS: [(&str, &str); 2] = [
    ("geosite-private", "geosite/private"),
    ("geoip-private", "geoip/private"),
];

/// 单个镜像的超时（下载 + 连接）。
const MIRROR_TIMEOUT: Duration = Duration::from_secs(10);

/// 内置规则集存放目录：`data_dir/rulesets/builtin`。
pub fn builtin_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("rulesets").join("builtin")
}

/// tag → 本地文件路径。
pub fn rule_set_path(data_dir: &Path, tag: &str) -> PathBuf {
    builtin_dir(data_dir).join(format!("{tag}.srs"))
}

/// 全部内置 tag（供报告 / 状态对齐）。
pub fn builtin_tags() -> Vec<&'static str> {
    BUILTIN_RULE_SETS.iter().map(|(tag, _)| *tag).collect()
}

/// 仅读取本地已有文件构成可用表（不发任何网络请求；配置预览等只读场景使用）。
#[must_use]
pub fn existing_rule_sets(data_dir: &Path) -> BTreeMap<String, PathBuf> {
    builtin_tags()
        .into_iter()
        .filter_map(|tag| {
            let path = rule_set_path(data_dir, tag);
            std::fs::metadata(&path)
                .is_ok_and(|m| m.len() > 0)
                .then(|| (tag.to_string(), path))
        })
        .collect()
}

/// 确保内置规则集本地可用：已存在（非空）直接使用；缺失按镜像回退下载（原子写入）。
///
/// 返回 `tag → 本地路径` 的可用表；下载全部镜像失败且无旧文件的 tag 不在表中（调用方
/// 按缺失降级）。逐个串行处理（5 个小文件，避免并发打满弱网）。
pub async fn ensure_builtin_rule_sets(
    data_dir: &Path,
    github_proxy_prefix: &str,
) -> BTreeMap<String, PathBuf> {
    let client = match reqwest::Client::builder()
        .timeout(MIRROR_TIMEOUT)
        .no_proxy()
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            tracing::warn!(error = %e, "构建规则集下载客户端失败，全部按缺失降级");
            return BTreeMap::new();
        }
    };
    ensure_with_downloader(data_dir, |rel: &str| {
        let rel = rel.to_string();
        let client = client.clone();
        async move {
            download_with_mirrors(&client, &rel, github_proxy_prefix)
                .await
                .map(|(bytes, _)| bytes)
        }
    })
    .await
}

/// 下载逻辑可注入的 ensure 实现（测试用桩替换真实镜像下载，消除网络依赖）。
async fn ensure_with_downloader<F, Fut>(data_dir: &Path, download: F) -> BTreeMap<String, PathBuf>
where
    F: Fn(&str) -> Fut,
    Fut: std::future::Future<Output = Result<bytes::Bytes, String>>,
{
    let mut available = BTreeMap::new();
    for (tag, rel) in BUILTIN_RULE_SETS {
        let path = rule_set_path(data_dir, tag);
        if std::fs::metadata(&path).is_ok_and(|m| m.len() > 0) {
            available.insert(tag.to_string(), path);
            continue;
        }
        match download(rel).await {
            Ok(bytes) if !bytes.is_empty() => match write_atomic(&path, &bytes) {
                Ok(()) => {
                    tracing::info!(tag, "内置规则集下载完成");
                    available.insert(tag.to_string(), path);
                }
                Err(e) => {
                    tracing::warn!(tag, error = %e, "内置规则集写入失败");
                }
            },
            Ok(_) => tracing::warn!(tag, "内置规则集下载为空，降级启动"),
            Err(e) => {
                tracing::warn!(tag, error = %e, "内置规则集全部镜像下载失败，降级启动");
            }
        }
    }
    available
}

/// 按镜像顺序下载单个规则集：jsDelivr → GitHub raw → GitHub 代理前缀 + GitHub raw
/// （前缀为空时跳过第三个镜像）。返回（内容，命中的镜像名）。
/// 镜像 URL 列表（按尝试顺序）：jsDelivr → GitHub raw →（有前缀时）GitHub 代理前缀 + raw。
fn mirror_urls(rel: &str, github_proxy_prefix: &str) -> Vec<(&'static str, String)> {
    let github_url =
        format!("https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/sing/geo/{rel}.srs");
    let mut mirrors = vec![
        (
            MIRROR_JSDELIVR,
            format!(
                "https://testingcf.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@sing/geo/{rel}.srs"
            ),
        ),
        (MIRROR_GITHUB, github_url.clone()),
    ];
    let prefixed = crate::apply_github_proxy_prefix(&github_url, github_proxy_prefix);
    if prefixed != github_url {
        mirrors.push(("github-proxy", prefixed));
    }
    mirrors
}

async fn download_with_mirrors(
    client: &reqwest::Client,
    rel: &str,
    github_proxy_prefix: &str,
) -> Result<(bytes::Bytes, &'static str), String> {
    let mirrors = mirror_urls(rel, github_proxy_prefix);
    let mut errors = Vec::new();
    for (name, url) in mirrors {
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => match resp.bytes().await {
                Ok(bytes) if !bytes.is_empty() => return Ok((bytes, name)),
                Ok(_) => errors.push(format!("{name}: 空响应")),
                Err(e) => errors.push(format!("{name}: {e}")),
            },
            Ok(resp) => errors.push(format!("{name}: HTTP {}", resp.status())),
            Err(e) => errors.push(format!("{name}: {e}")),
        }
    }
    Err(errors.join(" | "))
}

/// 原子写入（tmp + rename），避免半截文件被当作可用缓存。
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let Some(parent) = path.parent() else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "rule set path has no parent",
        ));
    };
    std::fs::create_dir_all(parent)?;
    let tmp = path.with_extension("srs.tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// 规则集物化报告（缺失 tag + 被移除的引用规则数）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MaterializeReport {
    /// 未能本地化的内置 tag（降级移除）。
    pub missing_tags: Vec<String>,
    /// 因引用缺失规则集而被移除的 `route.rules` 条数。
    pub stripped_route_rules: usize,
    /// 因引用缺失规则集而被移除的 `dns.rules` 条数。
    pub stripped_dns_rules: usize,
}

/// 对合成完成的配置做规则集物化（见模块文档方案第 2 步）。
///
/// - `route.rule_set` 中属于内置 5 tag 的条目：可用 → 改写为 `type: local`（保留 tag /
///   format，指向本地绝对路径）；缺失 → 移除条目；
/// - `route.rules` / `dns.rules` 中引用任一缺失 tag 的规则整条移除（降级）；
/// - 非内置 tag（用户自定义 local / remote 规则集）一律不动。
///
/// 必须在全部规则注入（模板 / 面板特性 / FakeIP）完成后调用，保证引用面完整。
pub fn materialize_rule_sets(
    config: &mut Value,
    data_dir: &Path,
    available: &BTreeMap<String, PathBuf>,
) -> MaterializeReport {
    let mut report = MaterializeReport::default();
    let builtin: Vec<String> = builtin_tags().into_iter().map(String::from).collect();
    let missing: Vec<String> = builtin
        .iter()
        .filter(|tag| !available.contains_key(*tag))
        .cloned()
        .collect();
    report.missing_tags = missing.clone();

    // route.rule_set：把 remote 内置条目改写为本地文件；仍不可用的 remote 条目移除。
    // 已经是 `type: local` 的条目（例如用户自行下载、与内置 tag 同名的自定义规则集）保持
    // 原样，避免覆盖用户数据。
    if let Some(arr) = config
        .pointer_mut("/route/rule_set")
        .and_then(Value::as_array_mut)
    {
        for entry in arr.iter_mut() {
            let Some(tag) = entry.get("tag").and_then(Value::as_str) else {
                continue;
            };
            if !is_remote_entry(entry) {
                continue;
            }
            if let Some(path) = available.get(tag) {
                *entry = serde_json::json!({
                    "type": "local",
                    "tag": tag,
                    "format": "binary",
                    "path": path.to_string_lossy(),
                });
            }
        }
        arr.retain(|entry| {
            let Some(tag) = entry.get("tag").and_then(Value::as_str) else {
                return true;
            };
            !(is_remote_entry(entry)
                && builtin.iter().any(|b| b == tag)
                && !available.contains_key(tag))
        });
    }

    // 引用缺失 tag 的规则整条移除（route.rules / dns.rules）。
    report.stripped_route_rules =
        strip_rules_referencing(config.pointer_mut("/route/rules"), &missing);
    report.stripped_dns_rules = strip_rules_referencing(config.pointer_mut("/dns/rules"), &missing);

    // 兜底：任何引用「route.rule_set 中不存在」的 tag 的规则都会被 sing-box 拒绝启动
    // （规则集可被用户删除、或尚未下载），这里统一剥离，核心仍然可起。
    let undefined = undefined_rule_set_tags(config);
    report.stripped_route_rules +=
        strip_rules_referencing(config.pointer_mut("/route/rules"), &undefined);
    report.stripped_dns_rules +=
        strip_rules_referencing(config.pointer_mut("/dns/rules"), &undefined);
    let _ = data_dir; // 路径已包含在 available 中；保留参数以稳定 API 语义
    report
}

/// 是否为 `type: remote` 的规则集条目（只有它需要被本地化物化）。
fn is_remote_entry(entry: &Value) -> bool {
    entry.get("type").and_then(Value::as_str) == Some("remote")
}

/// 收集 `route.rules` / `dns.rules` 中引用了「`route.rule_set` 未定义 tag」的规则集 tag。
fn undefined_rule_set_tags(config: &Value) -> Vec<String> {
    let defined: std::collections::HashSet<String> = config
        .pointer("/route/rule_set")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|entry| entry.get("tag").and_then(Value::as_str))
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();

    let mut tags: Vec<String> = Vec::new();
    for pointer in ["/route/rules", "/dns/rules"] {
        let Some(arr) = config.pointer(pointer).and_then(Value::as_array) else {
            continue;
        };
        for rule in arr {
            for tag in rule_set_tag_refs(rule) {
                if !defined.contains(&tag) && !tags.contains(&tag) {
                    tags.push(tag);
                }
            }
        }
    }
    tags
}

/// 单条规则的 `rule_set` 字段引用的全部 tag（字符串或数组）。
fn rule_set_tag_refs(rule: &Value) -> Vec<String> {
    match rule.get("rule_set") {
        Some(Value::String(tag)) => vec![tag.clone()],
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(Value::as_str)
            .map(String::from)
            .collect(),
        _ => Vec::new(),
    }
}

/// 移除 `rules` 数组中引用任一缺失 tag 的规则，返回移除条数。
fn strip_rules_referencing(rules: Option<&mut Value>, missing: &[String]) -> usize {
    if missing.is_empty() {
        return 0;
    }
    let Some(arr) = rules.and_then(Value::as_array_mut) else {
        return 0;
    };
    let before = arr.len();
    arr.retain(|rule| !rule_references_any(rule, missing));
    before - arr.len()
}

/// 规则的 `rule_set` 字段（字符串或数组）是否引用任一给定 tag。
fn rule_references_any(rule: &Value, tags: &[String]) -> bool {
    match rule.get("rule_set") {
        Some(Value::String(s)) => tags.iter().any(|t| t == s),
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(Value::as_str)
            .any(|s| tags.iter().any(|t| t == s)),
        _ => false,
    }
}

#[cfg(test)]
mod tests;
