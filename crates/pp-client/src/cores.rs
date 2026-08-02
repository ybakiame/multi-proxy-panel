//! 核心版本管理：下载管理本地核心 + 探测系统已装核心 + 启用选择。
//!
//! # 复用结论
//!
//! 未直接复用 `pp-core::installer` 的 `ensure_core_binary`：
//!
//! - 其语义面向 agent 的「单目录落盘 + 环境变量/latest 版本解析」，与本模块
//!   「`data_dir/cores/<core>/<version>/` 版本目录 + 显式版本」不同；
//! - 其 GitHub 域名固定，无法注入 mock 服务用于本地验收测试；
//! - 本模块保留其资产命名与解压思路，按客户端场景实现精简版。
//!
//! 远端版本与资产一律以 GitHub Release API 为准：真实资产命名可能偏离约定
//! （如 mihomo Alpha 通道按短 commit hash 命名），因此下载时先取 release 的
//! `assets` 列表按平台 / 架构匹配，而不是直接拼接下载 URL。

use std::path::{Path, PathBuf};
use std::process::Command;

use pp_common::{CoreType, PanelError, PanelResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::AsyncWriteExt;

use crate::config::ClientConfig;

/// GitHub API 请求超时（秒）。
const HTTP_TIMEOUT_SECS: u64 = 30;

/// 核心来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreSource {
    /// 由本模块从 GitHub Releases 下载到 `data_dir/cores`。
    Downloaded,
    /// 通过 PATH 探测到的系统已装核心。
    System,
}

/// 一个本地可用的核心（已下载或系统已装）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalCore {
    pub core_type: CoreType,
    pub version: String,
    pub path: PathBuf,
    pub source: CoreSource,
}

/// 客户端核心清单：管理下载目录扫描、远端版本列举、下载安装、系统探测与启用匹配。
#[derive(Debug, Clone)]
pub struct ClientCoreInventory {
    data_dir: PathBuf,
    api_base: String,
    client: reqwest::Client,
}

impl ClientCoreInventory {
    /// 基于数据目录创建清单（GitHub API 走 `https://api.github.com`）。
    pub fn new(data_dir: PathBuf) -> Self {
        Self::with_api_base(data_dir, "https://api.github.com")
    }

    /// 指定 GitHub API base（测试注入 mock 服务地址）。
    pub fn with_api_base(data_dir: PathBuf, api_base: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(HTTP_TIMEOUT_SECS))
            .no_proxy()
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            data_dir,
            api_base: api_base.into(),
            client,
        }
    }

    /// 下载目录：`data_dir/cores`。
    pub fn cores_dir(&self) -> PathBuf {
        self.data_dir.join("cores")
    }

    /// 指定核心版本目录：`data_dir/cores/<core>/<version>`。
    fn core_dir(&self, core_type: CoreType, version: &str) -> PathBuf {
        self.cores_dir().join(binary_name(core_type)).join(version)
    }

    /// 扫描 `cores_dir/<type>/<version>/`，列出已下载核心。
    pub fn list_installed(&self) -> Vec<LocalCore> {
        let mut out = Vec::new();
        for core_type in [CoreType::SingBox, CoreType::Mihomo] {
            let type_dir = self.cores_dir().join(binary_name(core_type));
            let Ok(entries) = std::fs::read_dir(&type_dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let version = entry.file_name().to_string_lossy().into_owned();
                let bin = path.join(binary_name_on_disk(core_type));
                if bin.is_file() {
                    out.push(LocalCore {
                        core_type,
                        version,
                        path: bin,
                        source: CoreSource::Downloaded,
                    });
                }
            }
        }
        out.sort_by(|a, b| {
            a.core_type
                .to_string()
                .cmp(&b.core_type.to_string())
                .then_with(|| a.version.cmp(&b.version))
                .then_with(|| a.path.cmp(&b.path))
        });
        out
    }

    /// 扫描 `cores_dir/<type>/<version>/` 版本目录，列出该核心类型已下载的版本号，
    /// 按语义化版本倒序（最新在前，预发布低于同基础稳定版）。
    pub fn list_downloaded_versions(&self, core_type: CoreType) -> Vec<String> {
        let mut versions: Vec<String> = self
            .list_installed()
            .into_iter()
            .filter(|c| c.core_type == core_type && c.source == CoreSource::Downloaded)
            .map(|c| c.version)
            .collect();
        versions.sort_by(|a, b| compare_core_versions(b, a));
        versions
    }

    /// 列举远端最近 10 个发布版本（去 `v` 前缀）。
    pub async fn list_remote_versions(&self, core_type: CoreType) -> PanelResult<Vec<String>> {
        let (owner, repo) = core_type.github_repo();
        let url = format!(
            "{}/repos/{}/{}/releases?per_page=10",
            self.api_base, owner, repo
        );
        let resp = self
            .client
            .get(&url)
            .header("User-Agent", "proxy-panel-client")
            .send()
            .await
            .map_err(|e| PanelError::Core(format!("GitHub API 请求失败: {e}")))?;
        if !resp.status().is_success() {
            return Err(PanelError::Core(format!(
                "GitHub API 返回状态 {}",
                resp.status()
            )));
        }
        let releases: Vec<Value> = resp
            .json()
            .await
            .map_err(|e| PanelError::Core(format!("解析 GitHub releases 失败: {e}")))?;
        let mut versions = Vec::new();
        for release in releases {
            if let Some(tag) = release.get("tag_name").and_then(|v| v.as_str()) {
                let version = tag.strip_prefix('v').unwrap_or(tag);
                if !version.is_empty() {
                    versions.push(version.to_string());
                }
            }
        }
        Ok(versions)
    }

    /// 下载指定版本核心并落盘到 `cores_dir/<type>/<version>/`。
    ///
    /// 已完成同版本下载时直接复用；下载后解压、chmod 755 并校验版本探测输出
    /// （`version` / `--version` / `-v` 依次尝试）包含目标版本。
    pub async fn download(&self, core_type: CoreType, version: &str) -> PanelResult<LocalCore> {
        let version = version.strip_prefix('v').unwrap_or(version).to_string();
        let tag = github_tag(&version);
        let (arch_hint, is_windows) = target_spec()?;

        let dir = self.core_dir(core_type, &version);
        let on_disk = dir.join(binary_name_on_disk(core_type));

        // 已下载且版本校验通过时直接复用。
        if on_disk.is_file() && verify_version(&on_disk, core_type, &version).is_ok() {
            return Ok(LocalCore {
                core_type,
                version,
                path: on_disk,
                source: CoreSource::Downloaded,
            });
        }

        tokio::fs::create_dir_all(&dir).await?;

        // 经 GitHub Release API 匹配当前平台资产（真实命名可能偏离约定，不拼 URL）。
        let (asset_url, asset_name) = self
            .resolve_asset_url(core_type, &tag, arch_hint, is_windows)
            .await
            .map_err(|e| {
                PanelError::Core(format!("解析 {} {} 发布资产失败: {e}", core_type, tag))
            })?;
        tracing::info!(
            core_type = %core_type,
            version = %version,
            url = %asset_url,
            "下载核心"
        );

        let tmp_archive = dir.join(format!(".download-{asset_name}"));
        self.download_to(&asset_url, &tmp_archive).await?;

        let binary_inside = binary_name(core_type);
        let (dir_clone, archive_clone) = (dir.clone(), tmp_archive.clone());
        let result = tokio::task::spawn_blocking(move || {
            if asset_name.ends_with(".tar.gz") {
                extract_tgz(&archive_clone, &dir_clone, binary_inside)
            } else if asset_name.ends_with(".zip") {
                extract_zip(&archive_clone, &dir_clone, binary_inside)
            } else if asset_name.ends_with(".gz") {
                extract_gzip(&archive_clone, &dir_clone, binary_inside)
            } else {
                Err(PanelError::Core(format!("未知资产格式: {asset_name}")))
            }
        })
        .await
        .map_err(|e| PanelError::Core(format!("解压任务失败: {e}")))??;
        // 尽力清理下载归档。
        let _ = std::fs::remove_file(&tmp_archive);

        let path = result;
        set_executable(&path)?;

        // 版本探测校验输出包含目标版本；失败时清理目录避免残留半成品。
        if let Err(e) = verify_version(&path, core_type, &version) {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(e);
        }

        tracing::info!(
            core_type = %core_type,
            version = %version,
            path = %path.display(),
            "核心下载完成"
        );
        Ok(LocalCore {
            core_type,
            version,
            path,
            source: CoreSource::Downloaded,
        })
    }

    /// 下载 `url` 到 `dest`（按 chunk 流式落盘）。
    async fn download_to(&self, url: &str, dest: &Path) -> PanelResult<()> {
        let mut resp = self
            .client
            .get(url)
            .header("User-Agent", "proxy-panel-client")
            .send()
            .await
            .map_err(|e| PanelError::Core(format!("下载请求失败: {e}")))?;
        if !resp.status().is_success() {
            return Err(PanelError::Core(format!(
                "下载返回状态 {}（{}）",
                resp.status(),
                url
            )));
        }
        let mut file = tokio::fs::File::create(dest).await?;
        while let Some(chunk) = resp
            .chunk()
            .await
            .map_err(|e| PanelError::Core(format!("下载流错误: {e}")))?
        {
            file.write_all(&chunk).await?;
        }
        Ok(())
    }

    /// 从 GitHub release 的 `assets` 中按平台 / 架构匹配资产，返回（下载 URL, 资产名）。
    async fn resolve_asset_url(
        &self,
        core_type: CoreType,
        tag: &str,
        arch_hint: &str,
        is_windows: bool,
    ) -> PanelResult<(String, String)> {
        let (owner, repo) = core_type.github_repo();
        let url = format!(
            "{}/repos/{}/{}/releases/tags/{}",
            self.api_base, owner, repo, tag
        );
        let resp = self
            .client
            .get(&url)
            .header("User-Agent", "proxy-panel-client")
            .send()
            .await
            .map_err(|e| PanelError::Core(format!("GitHub release 请求失败: {e}")))?;
        if !resp.status().is_success() {
            return Err(PanelError::Core(format!(
                "GitHub release 返回状态 {}",
                resp.status()
            )));
        }
        let release: Value = resp
            .json()
            .await
            .map_err(|e| PanelError::Core(format!("解析 GitHub release 失败: {e}")))?;
        let assets = release
            .get("assets")
            .and_then(|v| v.as_array())
            .ok_or_else(|| PanelError::Core("release 无 assets 字段".to_string()))?;

        for asset in assets {
            let name = asset.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if name.contains(arch_hint) && ext_ok(core_type, is_windows, name) {
                if let Some(url) = asset.get("browser_download_url").and_then(|v| v.as_str()) {
                    return Ok((url.to_string(), name.to_string()));
                }
            }
        }
        Err(PanelError::Core(format!(
            "在 release {tag} 中未找到 {} 平台资产",
            arch_hint
        )))
    }

    /// PATH 查找系统已装核心（`sing-box` / `mihomo`，Windows 追加 `.exe`），
    /// 依次尝试 `version` / `--version` / `-v` 探测并解析版本号；解析失败记为 `unknown`。
    pub fn detect_system_cores(&self) -> Vec<LocalCore> {
        let Some(path_value) = std::env::var_os("PATH") else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for dir in std::env::split_paths(&path_value) {
            if !dir.is_dir() {
                continue;
            }
            for core_type in [CoreType::SingBox, CoreType::Mihomo] {
                let candidate = dir.join(binary_name_on_disk(core_type));
                if !candidate.is_file() || out.iter().any(|c: &LocalCore| c.path == candidate) {
                    continue;
                }
                let version = parse_version_from_output(core_type, &binary_output(&candidate))
                    .unwrap_or_else(|| "unknown".to_string());
                out.push(LocalCore {
                    core_type,
                    version,
                    path: candidate,
                    source: CoreSource::System,
                });
            }
        }
        out
    }

    /// 按 `config.core_binary` 匹配已安装 / 系统核心；未设置时返回 `None`。
    pub fn active_core(&self, config: &ClientConfig) -> Option<LocalCore> {
        if config.core_binary.as_os_str().is_empty() {
            return None;
        }
        self.list_installed()
            .into_iter()
            .chain(self.detect_system_cores())
            .find(|c| paths_equal(&c.path, &config.core_binary))
    }

    /// 某核心类型的首选本地二进制：
    ///
    /// 1. 已下载核心中版本号最大的一个（语义化版本排序，预发布低于同基础的
    ///    稳定版，如 `1.14.0-beta.4` < `1.14.0` 但 `> 1.13.15`）；
    /// 2. 无已下载核心时回退到系统 PATH 探测到的该类型第一个核心；
    /// 3. 都没有 → `None`（命令层据此提示用户去核心管理下载）。
    pub fn preferred_binary(&self, core_type: CoreType) -> Option<PathBuf> {
        let downloaded = self
            .list_installed()
            .into_iter()
            .filter(|c| c.core_type == core_type)
            .max_by(|a, b| compare_core_versions(&a.version, &b.version));
        if let Some(core) = downloaded {
            return Some(core.path);
        }
        self.detect_system_cores()
            .into_iter()
            .find(|c| c.core_type == core_type)
            .map(|c| c.path)
    }

    /// 删除一个已下载核心（仅限 `cores_dir` 内的下载核心）。
    ///
    /// 删除 `cores/<type>/<version>/` 整个版本目录，类型目录删空后顺带清理。
    /// 错误：路径不在 `cores_dir` 内（系统核心）/ 路径不存在 / 该核心为
    /// `active_binary`（正在使用）。
    ///
    /// 安全约束：真实删除前先 `canonicalize` 校验路径归属，禁止删除 `cores_dir`
    /// 之外的任何内容（防目录穿越 / 符号链接逃逸）。
    pub fn delete(&self, path: &Path, active_binary: &Path) -> PanelResult<()> {
        // 防目录穿越：canonicalize 后确认目标位于下载目录之内。下载目录不存在时
        // 必然无已下载核心可删。
        let cores_dir = std::fs::canonicalize(self.cores_dir())
            .map_err(|_| PanelError::Core("核心下载目录不存在".to_string()))?;
        let bin = std::fs::canonicalize(path)
            .map_err(|e| PanelError::Core(format!("核心二进制不存在: {e}")))?;
        if !bin.starts_with(&cores_dir) {
            return Err(PanelError::Core(
                "系统核心不可删除：仅支持删除下载目录内的核心".to_string(),
            ));
        }
        // 结构校验：目标必须形如 `cores/<type>/<version>/<binary>`，避免误删
        // 类型目录甚至整个下载目录。
        if !bin.is_file() {
            return Err(PanelError::Core("无效的核心二进制路径".to_string()));
        }
        let version_dir = bin
            .parent()
            .ok_or_else(|| PanelError::Core("无法定位核心版本目录".to_string()))?;
        let type_dir = version_dir
            .parent()
            .ok_or_else(|| PanelError::Core("无法定位核心类型目录".to_string()))?;
        if type_dir.parent() != Some(cores_dir.as_path()) {
            return Err(PanelError::Core("无效的核心二进制路径".to_string()));
        }
        // 正在使用的核心不可删除。
        if paths_equal(&bin, active_binary) {
            return Err(PanelError::Core(
                "正在使用的核心不可删除：请先切换其他核心".to_string(),
            ));
        }
        tracing::info!(
            path = %bin.display(),
            version_dir = %version_dir.display(),
            "删除本地核心"
        );
        std::fs::remove_dir_all(version_dir)
            .map_err(|e| PanelError::Core(format!("删除核心失败: {e}")))?;
        // 类型目录（`cores/<type>/`）删空后顺带清理。
        if std::fs::read_dir(type_dir)
            .map(|mut it| it.next().is_none())
            .unwrap_or(false)
        {
            let _ = std::fs::remove_dir(type_dir);
        }
        Ok(())
    }
}

/// 核心目录 / 二进制基础名。
fn binary_name(core_type: CoreType) -> &'static str {
    match core_type {
        CoreType::SingBox => "sing-box",
        CoreType::Mihomo => "mihomo",
    }
}

/// 落盘二进制文件名（Windows 追加 `.exe`）。
fn binary_name_on_disk(core_type: CoreType) -> String {
    let base = binary_name(core_type);
    if std::env::consts::OS == "windows" {
        format!("{base}.exe")
    } else {
        base.to_string()
    }
}

/// 按文件名推断核心类型：文件名（忽略大小写）含 `sing-box` / `singbox` →
/// [`CoreType::SingBox`]，含 `mihomo` / `clash` → [`CoreType::Mihomo`]；
/// 无法识别时返回 `None`（命令层据此提示用户手动选择）。
///
/// 供命令层（`set_active_core` 未在清单中命中路径时的回退）与测试复用。
pub fn infer_core_type(path: &Path) -> Option<CoreType> {
    let name = path.file_name()?.to_string_lossy().to_lowercase();
    if name.contains("sing-box") || name.contains("singbox") {
        Some(CoreType::SingBox)
    } else if name.contains("mihomo") || name.contains("clash") {
        Some(CoreType::Mihomo)
    } else {
        None
    }
}

/// 当前平台资产提示：`("os-arch", is_windows)`。
fn target_spec() -> PanelResult<(&'static str, bool)> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok(("linux-amd64", false)),
        ("linux", "aarch64") => Ok(("linux-arm64", false)),
        ("macos", "x86_64") => Ok(("darwin-amd64", false)),
        ("macos", "aarch64") => Ok(("darwin-arm64", false)),
        ("windows", "x86_64") => Ok(("windows-amd64", true)),
        ("windows", "aarch64") => Ok(("windows-arm64", true)),
        (os, arch) => Err(PanelError::Core(format!(
            "不支持的核心下载平台: {os}-{arch}"
        ))),
    }
}

/// 把版本号规范为 GitHub tag（稳定版加 `v` 前缀；mihomo Alpha 通道等自有前缀保持原样）。
fn github_tag(version: &str) -> String {
    let prefixed = version.starts_with('v')
        || version.starts_with("Alpha")
        || version.starts_with("alpha")
        || version.starts_with("Release")
        || version.starts_with("release");
    if prefixed {
        version.to_string()
    } else {
        format!("v{version}")
    }
}

/// 资产扩展名是否匹配当前核心 / 平台。
fn ext_ok(core_type: CoreType, is_windows: bool, name: &str) -> bool {
    if is_windows {
        name.ends_with(".zip")
    } else {
        match core_type {
            CoreType::SingBox => name.ends_with(".tar.gz"),
            CoreType::Mihomo => name.ends_with(".gz") && !name.ends_with(".tar.gz"),
        }
    }
}

/// 依次尝试 `version` 子命令 / `--version` / `-v`，取第一个退出码为 0 的输出
/// （拼接 stdout / stderr）。
///
/// sing-box 1.14+ 移除了 `--version` flag，改用 `version` 子命令；mihomo 传统上
/// 支持 `-v`。统一按 `version` → `--version` → `-v` 顺序探测，兼容新旧核心。
fn binary_output(binary: &Path) -> String {
    for arg in ["version", "--version", "-v"] {
        if let Ok(output) = Command::new(binary).arg(arg).output() {
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            if output.status.success() {
                return text;
            }
        }
    }
    String::new()
}

/// 从版本探测输出解析版本号。
///
/// sing-box: `sing-box version 1.13.15`；`version` 子命令输出可能含多行
/// （首行 `sing-box version 1.14.0-beta.4` + 环境信息），取含 "version" 的首行。
/// mihomo:   `Mihomo Meta v1.19.29 linux/amd64 go1.23.4`
fn parse_version_from_output(core_type: CoreType, output: &str) -> Option<String> {
    let pattern = match core_type {
        CoreType::SingBox => r"sing-box\s+version\s+v?([0-9][0-9A-Za-z.\-]*)",
        CoreType::Mihomo => r"(?i)mihomo[^\n]*?\bv?([0-9][0-9A-Za-z.\-]*)",
    };
    let re = regex::Regex::new(pattern).ok()?;
    // 子命令输出可能含多行：取含 "version" 的首行优先匹配（sing-box 1.14+），
    // 其余格式（如 mihomo 单行）回退到全文匹配。
    if let Some(line) = output.lines().find(|l| l.contains("version")) {
        if let Some(m) = re.captures(line).and_then(|c| c.get(1)) {
            return Some(m.as_str().to_string());
        }
    }
    re.captures(output)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

/// 校验版本探测输出包含目标版本（允许 `v` 前缀，或解析出的版本号相等）。
fn verify_version(binary: &Path, core_type: CoreType, version: &str) -> PanelResult<()> {
    let text = binary_output(binary);
    let parsed = parse_version_from_output(core_type, &text).unwrap_or_default();
    if !version.is_empty()
        && (text.contains(version) || text.contains(&format!("v{version}")) || parsed == version)
    {
        return Ok(());
    }
    Err(PanelError::Core(format!(
        "核心 {core_type} 版本校验失败：请求 {version}，版本探测输出：{text}"
    )))
}

/// 解压 `.tar.gz` 并取目标二进制。
fn extract_tgz(archive: &Path, dest_dir: &Path, target_name: &str) -> PanelResult<PathBuf> {
    let file = std::fs::File::open(archive)?;
    let tar = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(tar);
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if file_name == target_name || file_name == format!("{target_name}.exe") {
            let dest = dest_dir.join(binary_name_on_disk(core_type_from_name(target_name)?));
            entry.unpack(&dest)?;
            return Ok(dest);
        }
    }
    Err(PanelError::Core(format!(
        "二进制 {target_name} 未在归档中找到"
    )))
}

/// 解压 `.zip` 并取目标二进制。
fn extract_zip(archive: &Path, dest_dir: &Path, target_name: &str) -> PanelResult<PathBuf> {
    let file = std::fs::File::open(archive)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| PanelError::Core(format!("无效的 zip 归档: {e}")))?;
    let mut binary_dest: Option<PathBuf> = None;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| PanelError::Core(format!("zip 条目错误: {e}")))?;
        let file_name = std::path::Path::new(entry.name())
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if file_name.eq_ignore_ascii_case(target_name)
            || file_name.eq_ignore_ascii_case(&format!("{target_name}.exe"))
        {
            let dest = dest_dir.join(binary_name_on_disk(core_type_from_name(target_name)?));
            let mut out = std::fs::File::create(&dest)?;
            std::io::copy(&mut entry, &mut out)?;
            binary_dest = Some(dest);
        }
    }
    binary_dest.ok_or_else(|| PanelError::Core(format!("二进制 {target_name} 未在归档中找到")))
}

/// 解压单个 gzip 文件（mihomo 非 Windows 资产为单二进制 gz）。
fn extract_gzip(archive: &Path, dest_dir: &Path, target_name: &str) -> PanelResult<PathBuf> {
    let file = std::fs::File::open(archive)?;
    let mut decoder = flate2::read::GzDecoder::new(file);
    let dest = dest_dir.join(binary_name_on_disk(core_type_from_name(target_name)?));
    let mut out = std::fs::File::create(&dest)?;
    std::io::copy(&mut decoder, &mut out)?;
    Ok(dest)
}

fn core_type_from_name(name: &str) -> PanelResult<CoreType> {
    match name {
        "sing-box" => Ok(CoreType::SingBox),
        "mihomo" => Ok(CoreType::Mihomo),
        _ => Err(PanelError::Core(format!("未知核心名: {name}"))),
    }
}

/// chmod 755（Unix）。
fn set_executable(path: &Path) -> PanelResult<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)?.permissions();
        perms.set_mode(perms.mode() | 0o755);
        std::fs::set_permissions(path, perms)?;
    }
    Ok(())
}

/// 宽松路径相等：直接比较，或都解析真实路径后比较。
fn paths_equal(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => false,
    }
}

/// 语义化版本比较（供 [`ClientCoreInventory::preferred_binary`] 排序已下载核心）。
///
/// 约定：
/// - 数字段（`.` 分隔）按数值比较：`1.14.0` > `1.13.15`；
/// - 数字段相同但带预发布后缀的版本低于同基础稳定版（`1.14.0-beta.4` < `1.14.0`）；
/// - mihomo 自命名通道（`Alpha-` / `Release-` 前缀，如 `Alpha-1.19.30`）按「前缀标记为
///   预发布 + 后续数字段数值」参与排序；
/// - 无法解析出版本段的字符串（如 `unknown`）按空版本段处理 → 视为最旧。
fn compare_core_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let (a_pre, a_core) = split_version_identity(a);
    let (b_pre, b_core) = split_version_identity(b);

    let a_nums = parse_numeric_segments(&a_core);
    let b_nums = parse_numeric_segments(&b_core);
    // 按数字段逐段比较；先达到段尾且数字相等者更小（1.14 < 1.14.0）。
    for (x, y) in a_nums.iter().zip(b_nums.iter()) {
        match x.cmp(y) {
            std::cmp::Ordering::Equal => {}
            other => return other,
        }
    }
    let len_cmp = a_nums.len().cmp(&b_nums.len());
    if len_cmp != std::cmp::Ordering::Equal {
        return len_cmp;
    }
    // 数字段完全相等：稳定版 > 预发布；预发布之间按通道前缀 + 尾段兜底。
    match (a_pre, b_pre) {
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, None) => std::cmp::Ordering::Equal,
        (Some(p), Some(q)) => p.cmp(&q).then_with(|| a_core.cmp(&b_core)),
    }
}

/// 拆出版本标识的（预发布前缀, 数字主干）。
///
/// `1.14.0-beta.4` → `(Some("beta.4"), "1.14.0")`；
/// `Alpha-1.19.30` → `(Some("alpha"), "1.19.30")`；
/// `1.13.15` → `(None, "1.13.15")`。
fn split_version_identity(v: &str) -> (Option<String>, String) {
    let v = v.trim();
    // mihomo 自有前缀通道（`Alpha` / `Release`，大小写不敏感）。
    if let Some(rest) = v
        .strip_prefix("Alpha-")
        .or_else(|| v.strip_prefix("alpha-"))
        .or_else(|| v.strip_prefix("Release-"))
        .or_else(|| v.strip_prefix("release-"))
    {
        return (Some("alpha".to_string()), rest.to_string());
    }
    // 标准 `数字[.数字].prerelease`：`-` 后为预发布标记。
    if let Some(idx) = v.find('-') {
        let (core, pre) = v.split_at(idx);
        let pre = pre.trim_start_matches('-');
        if !core.is_empty() && core.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            return (Some(pre.to_ascii_lowercase()), core.to_string());
        }
    }
    (None, v.to_string())
}

/// 解析 `.` 分隔的数字段（非数字段截断忽略）。
fn parse_numeric_segments(s: &str) -> Vec<u64> {
    s.split('.')
        .map(|seg| seg.trim_end_matches(|c: char| !c.is_ascii_digit()))
        .filter(|seg| !seg.is_empty())
        .filter_map(|seg| seg.parse::<u64>().ok())
        .collect()
}

#[cfg(test)]
mod version_tests {
    use super::compare_core_versions;
    use std::cmp::Ordering;

    #[test]
    fn compares_numeric_segments() {
        assert_eq!(compare_core_versions("1.13.15", "1.14.0"), Ordering::Less);
        assert_eq!(
            compare_core_versions("1.14.0", "1.13.15"),
            Ordering::Greater
        );
        assert_eq!(compare_core_versions("1.14.0", "1.14.0"), Ordering::Equal);
        // 段尾语义：1.14 < 1.14.0。
        assert_eq!(compare_core_versions("1.14", "1.14.0"), Ordering::Less);
    }

    #[test]
    fn prerelease_sorts_below_same_base_stable() {
        assert_eq!(
            compare_core_versions("1.14.0-beta.4", "1.14.0"),
            Ordering::Less
        );
        assert_eq!(
            compare_core_versions("1.14.0", "1.14.0-beta.4"),
            Ordering::Greater
        );
        // 预发布基础版本高于旧稳定版：1.14.0-beta.4 > 1.13.15。
        assert_eq!(
            compare_core_versions("1.13.15", "1.14.0-beta.4"),
            Ordering::Less
        );
    }

    #[test]
    fn mihomo_alpha_channel_sorts_by_numeric_after_prefix() {
        assert_eq!(
            compare_core_versions("Alpha-1.19.30", "1.19.29"),
            Ordering::Greater
        );
        assert_eq!(
            compare_core_versions("Alpha-1.19.30", "1.19.30"),
            Ordering::Less
        );
    }

    #[test]
    fn unknown_versions_fallback_lowest() {
        // 无法解析出版本段（如 `unknown`）按空版本段处理 → 视为最旧，低于真实
        // 版本；两个 unknown 之间相等。
        assert_eq!(compare_core_versions("unknown", "1.19.29"), Ordering::Less);
        assert_eq!(compare_core_versions("unknown", "unknown"), Ordering::Equal);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn spawn_server(app: axum::Router) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    fn write_executable(path: &Path, content: &[u8]) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    /// 全局 PATH 锁：环境变量是进程级状态，并行测试间互斥，避免相互串台。
    static PATH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// 在指定 PATH 下执行闭包（互斥串行化，测试内单线程修改/恢复环境变量）。
    fn with_patched_path<T>(path: &Path, f: impl FnOnce() -> T) -> T {
        let _guard = PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let old = std::env::var_os("PATH");
        // Rust 2024 下 std::env 的 set_var/remove_var 标记为 unsafe（并发修改
        // 环境变量是未定义行为），PATH_LOCK 保证测试进程内串行访问。
        unsafe {
            std::env::set_var("PATH", path);
        }
        let result = f();
        match old {
            Some(v) => unsafe { std::env::set_var("PATH", v) },
            None => unsafe { std::env::remove_var("PATH") },
        }
        result
    }

    /// 构造含若干条目的 tar.gz。
    fn build_tgz(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let enc = flate2::write::GzEncoder::new(&mut out, flate2::Compression::default());
            let mut tar = tar::Builder::new(enc);
            for (name, data) in entries {
                let mut header = tar::Header::new_gnu();
                header.set_size(data.len() as u64);
                header.set_mode(0o755);
                header.set_path(name).unwrap();
                tar.append_data(&mut header, name, *data).unwrap();
            }
            tar.into_inner().unwrap().finish().unwrap();
        }
        out
    }

    /// gzip 压缩单文件（mihomo 非 Windows 资产形态）。
    fn gzip_bytes(data: &[u8]) -> Vec<u8> {
        use std::io::Write;
        let mut out = Vec::new();
        let mut enc = flate2::write::GzEncoder::new(&mut out, flate2::Compression::default());
        enc.write_all(data).unwrap();
        enc.finish().unwrap();
        out
    }

    // ---------- ① list_installed：扫描目录结构 ----------

    #[test]
    fn list_installed_scans_versioned_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let inv = ClientCoreInventory::new(dir.path().to_path_buf());

        write_executable(&dir.path().join("cores/sing-box/1.13.15/sing-box"), b"fake");
        write_executable(&dir.path().join("cores/mihomo/1.19.29/mihomo"), b"fake");
        // 无二进制文件的版本目录应被跳过。
        std::fs::create_dir_all(dir.path().join("cores/sing-box/1.12.0")).unwrap();

        let cores = inv.list_installed();
        assert_eq!(cores.len(), 2);
        let sb = cores
            .iter()
            .find(|c| c.core_type == CoreType::SingBox)
            .unwrap();
        assert_eq!(sb.version, "1.13.15");
        assert_eq!(sb.source, CoreSource::Downloaded);
        assert_eq!(sb.path, dir.path().join("cores/sing-box/1.13.15/sing-box"));
        let mh = cores
            .iter()
            .find(|c| c.core_type == CoreType::Mihomo)
            .unwrap();
        assert_eq!(mh.version, "1.19.29");
        assert_eq!(mh.source, CoreSource::Downloaded);
    }

    #[test]
    fn list_downloaded_versions_sorts_semantically_descending() {
        let dir = tempfile::tempdir().unwrap();
        let inv = ClientCoreInventory::new(dir.path().to_path_buf());

        // 语义化倒序：1.14.0 > 1.14.0-beta.4 > 1.13.15。
        for v in ["1.13.15", "1.14.0-beta.4", "1.14.0"] {
            write_executable(
                &dir.path().join(format!("cores/sing-box/{v}/sing-box")),
                b"fake",
            );
        }
        // 无二进制文件的版本目录不计入。
        std::fs::create_dir_all(dir.path().join("cores/sing-box/1.12.0")).unwrap();
        // 其他核心类型不计入。
        write_executable(&dir.path().join("cores/mihomo/1.19.29/mihomo"), b"fake");

        assert_eq!(
            inv.list_downloaded_versions(CoreType::SingBox),
            vec!["1.14.0", "1.14.0-beta.4", "1.13.15"]
        );
        assert_eq!(
            inv.list_downloaded_versions(CoreType::Mihomo),
            vec!["1.19.29"]
        );
        assert!(
            inv.list_downloaded_versions(CoreType::SingBox)
                .into_iter()
                .all(|v| !v.starts_with("1.12"))
        );
    }

    // ---------- ② list_remote_versions：mock releases API ----------
    #[tokio::test]
    async fn list_remote_versions_parses_both_cores() {
        let singbox_releases = serde_json::json!([
            { "tag_name": "v1.13.15" },
            { "tag_name": "v1.13.14" },
            { "tag_name": "v1.12.0-alpha.1" },
        ]);
        let mihomo_releases = serde_json::json!([
            { "tag_name": "v1.19.29" },
            { "tag_name": "Alpha-1.19.30" },
            { "tag_name": "v1.19.28" },
        ]);
        let app = axum::Router::new()
            .route(
                "/repos/SagerNet/sing-box/releases",
                axum::routing::get(move || async move { singbox_releases.to_string() }),
            )
            .route(
                "/repos/MetaCubeX/mihomo/releases",
                axum::routing::get(move || async move { mihomo_releases.to_string() }),
            );
        let base = spawn_server(app).await;
        let inv = ClientCoreInventory::with_api_base(PathBuf::new(), &base);

        let sb = inv.list_remote_versions(CoreType::SingBox).await.unwrap();
        assert_eq!(sb, vec!["1.13.15", "1.13.14", "1.12.0-alpha.1"]);

        let mh = inv.list_remote_versions(CoreType::Mihomo).await.unwrap();
        assert_eq!(mh, vec!["1.19.29", "Alpha-1.19.30", "1.19.28"]);
    }

    // ---------- ③ download：mock asset 下载 + 解压 + chmod + --version ----------
    //
    // 假二进制是 shell 脚本，仅 Unix 可运行；mock 的 release 响应通过 axum
    // `Host` 提取器回填 `browser_download_url`，避免 base URL 闭包捕获顺序问题。

    #[cfg(unix)]
    #[tokio::test]
    async fn download_singbox_targz_extracts_and_verifies() {
        let (arch_hint, is_windows) = target_spec().unwrap();
        let ext = if is_windows { "zip" } else { "tar.gz" };
        let asset = format!("sing-box-1.13.15-{arch_hint}.{ext}");
        let fake: &[u8] = b"#!/bin/sh\necho 'sing-box version 1.13.15'\n";
        let body = build_tgz(&[("sing-box", fake)]);

        let asset_for_release = asset.clone();
        let app = axum::Router::new()
            .route(
                "/repos/SagerNet/sing-box/releases/tags/v1.13.15",
                axum::routing::get(move |headers: axum::http::HeaderMap| async move {
                    let host = headers
                        .get("host")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("127.0.0.1");
                    serde_json::json!({
                        "tag_name": "v1.13.15",
                        "assets": [
                            { "name": asset_for_release.clone(),
                              "browser_download_url":
                                  format!("http://{host}/assets/{asset_for_release}") },
                        ],
                    })
                    .to_string()
                }),
            )
            .route(
                &format!("/assets/{asset}"),
                axum::routing::get(move || async move { body.clone() }),
            );
        let base = spawn_server(app).await;

        let dir = tempfile::tempdir().unwrap();
        let inv = ClientCoreInventory::with_api_base(dir.path().to_path_buf(), &base);
        let core = inv.download(CoreType::SingBox, "1.13.15").await.unwrap();

        assert_eq!(core.core_type, CoreType::SingBox);
        assert_eq!(core.version, "1.13.15");
        assert_eq!(core.source, CoreSource::Downloaded);
        assert_eq!(
            core.path,
            dir.path().join("cores/sing-box/1.13.15/sing-box")
        );
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&core.path).unwrap().permissions().mode();
            assert_ne!(mode & 0o111, 0, "二进制应为可执行");
        }
        // 再次下载命中缓存。
        let again = inv.download(CoreType::SingBox, "1.13.15").await.unwrap();
        assert_eq!(again.path, core.path);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn download_mihomo_single_gz_extracts_and_verifies() {
        let (arch_hint, is_windows) = target_spec().unwrap();
        let ext = if is_windows { "zip" } else { "gz" };
        let asset = format!("mihomo-{arch_hint}-v1.19.29.{ext}");
        let fake: &[u8] = b"#!/bin/sh\necho 'Mihomo Meta v1.19.29'\n";
        let body = gzip_bytes(fake);

        let asset_for_release = asset.clone();
        let app = axum::Router::new()
            .route(
                "/repos/MetaCubeX/mihomo/releases/tags/v1.19.29",
                axum::routing::get(move |headers: axum::http::HeaderMap| async move {
                    let host = headers
                        .get("host")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("127.0.0.1");
                    serde_json::json!({
                        "tag_name": "v1.19.29",
                        "assets": [
                            { "name": asset_for_release.clone(),
                              "browser_download_url":
                                  format!("http://{host}/assets/{asset_for_release}") },
                        ],
                    })
                    .to_string()
                }),
            )
            .route(
                &format!("/assets/{asset}"),
                axum::routing::get(move || async move { body.clone() }),
            );
        let base = spawn_server(app).await;

        let dir = tempfile::tempdir().unwrap();
        let inv = ClientCoreInventory::with_api_base(dir.path().to_path_buf(), &base);
        let core = inv.download(CoreType::Mihomo, "1.19.29").await.unwrap();

        assert_eq!(core.version, "1.19.29");
        assert_eq!(core.path, dir.path().join("cores/mihomo/1.19.29/mihomo"));
        assert!(core.path.is_file());
    }

    // ---------- ④ detect_system_cores：临时目录造假二进制入 PATH ----------

    #[test]
    fn detect_system_cores_finds_core_on_path() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("sing-box");
        write_executable(&bin, b"#!/bin/sh\necho 'sing-box version 1.19.9'\n");

        // PATH 由 with_patched_path 加锁串行修改/恢复，避免与并行测试竞争。
        let result = with_patched_path(dir.path(), || {
            let inv = ClientCoreInventory::new(PathBuf::new());
            inv.detect_system_cores()
        });

        let found = result
            .iter()
            .find(|c| c.core_type == CoreType::SingBox && c.path == bin);
        assert!(found.is_some(), "应在 PATH 中发现假 sing-box");
        assert_eq!(found.unwrap().version, "1.19.9");
        assert_eq!(found.unwrap().source, CoreSource::System);
    }

    // ---------- ⑤ 版本探测：`version` / `--version` / `-v` 三形态 ----------
    //
    // 三种假二进制分别只支持 `version` 子命令（sing-box 1.14+，含多行输出）、
    // `--version` flag（旧 sing-box）、`-v`（mihomo），断言探测与解析均成功。

    #[test]
    fn version_probe_supports_subcommand_and_flags() {
        let dir = tempfile::tempdir().unwrap();

        // sing-box 1.14+：仅支持 `version` 子命令，`--version` 报错退出非零；
        // 子命令输出含多行（版本行 + 环境信息）。
        let subcmd = dir.path().join("sing-box-subcmd");
        write_executable(
            &subcmd,
            b"#!/bin/sh\n\
              [ \"$1\" = \"version\" ] || { echo 'Error: unknown flag: --version' >&2; exit 1; }\n\
              echo 'sing-box version 1.14.0-beta.4'\n\
              echo\n\
              echo 'Environment:'\n\
              echo '  go version go1.24.3'\n",
        );

        // 旧 sing-box：仅支持 `--version` flag。
        let flag = dir.path().join("sing-box-flag");
        write_executable(&flag, b"#!/bin/sh\necho 'sing-box version 1.13.15'\n");

        // mihomo：仅支持 `-v`。
        let mihomo = dir.path().join("mihomo-v");
        write_executable(
            &mihomo,
            b"#!/bin/sh\n\
              [ \"$1\" = \"-v\" ] || exit 1\n\
              echo 'Mihomo Meta v1.19.29 linux/amd64 go1.23.4'\n",
        );

        // download 后校验路径：三种形态均探测成功。
        verify_version(&subcmd, CoreType::SingBox, "1.14.0-beta.4").unwrap();
        verify_version(&flag, CoreType::SingBox, "1.13.15").unwrap();
        verify_version(&mihomo, CoreType::Mihomo, "1.19.29").unwrap();

        // detect_system_cores 路径：输出解析正确。
        assert_eq!(
            parse_version_from_output(CoreType::SingBox, &binary_output(&subcmd)),
            Some("1.14.0-beta.4".to_string())
        );
        assert_eq!(
            parse_version_from_output(CoreType::SingBox, &binary_output(&flag)),
            Some("1.13.15".to_string())
        );
        assert_eq!(
            parse_version_from_output(CoreType::Mihomo, &binary_output(&mihomo)),
            Some("1.19.29".to_string())
        );
    }

    // ---------- ⑥ active_core：按 config.core_binary 匹配 ----------

    #[test]
    fn active_core_matches_config_binary() {
        let dir = tempfile::tempdir().unwrap();
        write_executable(
            &dir.path().join("cores/sing-box/1.13.15/sing-box"),
            b"#!/bin/sh\necho 'sing-box version 1.13.15'\n",
        );
        let inv = ClientCoreInventory::new(dir.path().to_path_buf());
        let bin = dir.path().join("cores/sing-box/1.13.15/sing-box");

        let mut cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            "http://127.0.0.1:50052",
            "tok",
            CoreType::SingBox,
            bin.clone(),
        );
        let active = inv.active_core(&cfg);
        assert!(active.is_some());
        assert_eq!(active.unwrap().path, bin);

        // 不匹配的路径 → None。
        cfg.core_binary = PathBuf::from("/nonexistent/sing-box");
        assert!(inv.active_core(&cfg).is_none());

        // 空路径 → None。
        cfg.core_binary = PathBuf::new();
        assert!(inv.active_core(&cfg).is_none());
    }

    // ---------- ⑦ infer_core_type：文件名推断 ----------

    #[test]
    fn infers_core_type_from_file_name() {
        assert_eq!(
            infer_core_type(Path::new("/usr/local/bin/sing-box")),
            Some(CoreType::SingBox)
        );
        assert_eq!(
            infer_core_type(Path::new("C:\\cores\\sing-box.exe")),
            Some(CoreType::SingBox)
        );
        assert_eq!(
            infer_core_type(Path::new("/usr/local/bin/singbox")),
            Some(CoreType::SingBox)
        );
        assert_eq!(
            infer_core_type(Path::new("/usr/local/bin/mihomo")),
            Some(CoreType::Mihomo)
        );
        assert_eq!(
            infer_core_type(Path::new("C:\\cores\\clash.exe")),
            Some(CoreType::Mihomo)
        );
        assert_eq!(infer_core_type(Path::new("/usr/bin/unknown")), None);
    }

    // ---------- 辅助：版本解析 ----------

    #[test]
    fn parses_version_from_output() {
        assert_eq!(
            parse_version_from_output(CoreType::SingBox, "sing-box version 1.13.15"),
            Some("1.13.15".to_string())
        );
        assert_eq!(
            parse_version_from_output(
                CoreType::SingBox,
                "sing-box version 1.14.0-beta.4\n\nEnvironment:\n  go version go1.24.3"
            ),
            Some("1.14.0-beta.4".to_string())
        );
        assert_eq!(
            parse_version_from_output(
                CoreType::Mihomo,
                "Mihomo Meta v1.19.29 linux/amd64 go1.23.4"
            ),
            Some("1.19.29".to_string())
        );
    }

    // ---------- ⑧ preferred_binary：已下载版本排序 + 系统回退 ----------

    #[test]
    fn preferred_binary_picks_newest_downloaded_version() {
        let dir = tempfile::tempdir().unwrap();
        write_executable(
            &dir.path().join("cores/sing-box/1.13.15/sing-box"),
            b"#!/bin/sh\necho 'sing-box version 1.13.15'\n",
        );
        write_executable(
            &dir.path().join("cores/sing-box/1.14.0-beta.4/sing-box"),
            b"#!/bin/sh\necho 'sing-box version 1.14.0-beta.4'\n",
        );
        let inv = ClientCoreInventory::new(dir.path().to_path_buf());

        let bin = inv.preferred_binary(CoreType::SingBox);
        // 语义化版本排序：1.14.0-beta.4（基础 1.14.0）> 1.13.15。
        assert_eq!(
            bin,
            Some(dir.path().join("cores/sing-box/1.14.0-beta.4/sing-box"))
        );
    }

    #[test]
    fn preferred_binary_falls_back_to_system_core() {
        let dir = tempfile::tempdir().unwrap();
        // 无任何已下载核心 → 回退系统 PATH 探测。
        let system_bin = dir.path().join("mihomo");
        write_executable(&system_bin, b"#!/bin/sh\necho 'Mihomo Meta v1.19.29'\n");
        let inv = ClientCoreInventory::new(dir.path().join("cores"));

        let result = with_patched_path(dir.path(), || inv.preferred_binary(CoreType::Mihomo));
        assert_eq!(result, Some(system_bin));
    }

    #[test]
    fn preferred_binary_none_when_unavailable() {
        let dir = tempfile::tempdir().unwrap();
        let inv = ClientCoreInventory::new(dir.path().to_path_buf());
        let result = with_patched_path(Path::new("/nonexistent-bin-dir"), || {
            inv.preferred_binary(CoreType::SingBox)
        });
        assert_eq!(result, None);
    }

    // ---------- ⑨ delete：本地核心删除 ----------

    #[test]
    fn delete_removes_version_dir_keeps_other_versions() {
        let dir = tempfile::tempdir().unwrap();
        let inv = ClientCoreInventory::new(dir.path().to_path_buf());

        let bin = dir.path().join("cores/sing-box/1.13.15/sing-box");
        write_executable(&bin, b"fake");
        // 同类型其他版本保留；active 指向它（而非被删目标）。
        let other = dir.path().join("cores/sing-box/1.14.0/sing-box");
        write_executable(&other, b"fake");

        inv.delete(&bin, &other).unwrap();

        assert!(!bin.exists(), "二进制应被删除");
        assert!(
            !dir.path().join("cores/sing-box/1.13.15").exists(),
            "版本目录应整体删除"
        );
        // 类型目录保留（还有其他版本），其他版本不受影响。
        assert!(other.exists(), "其他版本应保留");
        assert!(dir.path().join("cores/sing-box").is_dir());
    }

    #[test]
    fn delete_prunes_empty_type_dir() {
        let dir = tempfile::tempdir().unwrap();
        let inv = ClientCoreInventory::new(dir.path().to_path_buf());
        let bin = dir.path().join("cores/mihomo/1.19.29/mihomo");
        write_executable(&bin, b"fake");

        inv.delete(&bin, Path::new("/nonexistent/other")).unwrap();

        // 版本目录与类型目录均被清理；cores 目录本身保留。
        assert!(!dir.path().join("cores/mihomo").exists());
        assert!(dir.path().join("cores").is_dir());
    }

    #[test]
    fn delete_rejects_path_outside_cores_dir() {
        let dir = tempfile::tempdir().unwrap();
        let inv = ClientCoreInventory::new(dir.path().to_path_buf());
        // cores 目录存在，但目标在其外（系统核心语义）。
        std::fs::create_dir_all(dir.path().join("cores")).unwrap();
        let system_bin = dir.path().join("bin/sing-box");
        write_executable(&system_bin, b"fake");

        let err = inv
            .delete(&system_bin, Path::new("/nonexistent/active"))
            .unwrap_err();
        assert!(
            err.to_string().contains("系统核心不可删除"),
            "应拒绝系统路径: {err}"
        );
        assert!(system_bin.exists(), "系统核心不应被删除");
    }

    #[test]
    fn delete_rejects_active_binary() {
        let dir = tempfile::tempdir().unwrap();
        let inv = ClientCoreInventory::new(dir.path().to_path_buf());
        let bin = dir.path().join("cores/sing-box/1.13.15/sing-box");
        write_executable(&bin, b"fake");

        let err = inv.delete(&bin, &bin).unwrap_err();
        assert!(
            err.to_string().contains("正在使用的核心不可删除"),
            "应拒绝使用中的核心: {err}"
        );
        assert!(bin.exists(), "active 核心不应被删除");
    }

    #[test]
    fn delete_rejects_nonexistent_path() {
        let dir = tempfile::tempdir().unwrap();
        let inv = ClientCoreInventory::new(dir.path().to_path_buf());
        let missing = dir.path().join("cores/sing-box/9.9.9/sing-box");

        let err = inv
            .delete(&missing, Path::new("/nonexistent/active"))
            .unwrap_err();
        assert!(err.to_string().contains("不存在"), "应报路径不存在: {err}");
    }

    #[test]
    fn delete_rejects_directory_under_cores_dir() {
        // 传入类型目录/版本目录本身（非二进制文件）应被拒绝，防止误删更大范围。
        let dir = tempfile::tempdir().unwrap();
        let inv = ClientCoreInventory::new(dir.path().to_path_buf());
        let version_dir = dir.path().join("cores/sing-box/1.13.15/sing-box");
        write_executable(&version_dir, b"fake");
        std::fs::create_dir_all(dir.path().join("cores/mihomo/1.19.29")).unwrap();

        // 类型目录（无二进制）不可删。
        let err = inv
            .delete(
                &dir.path().join("cores/mihomo"),
                Path::new("/nonexistent/active"),
            )
            .unwrap_err();
        assert!(
            err.to_string().contains("无效的核心二进制路径"),
            "应拒绝目录: {err}"
        );
        assert!(dir.path().join("cores/mihomo").is_dir());

        // 版本目录（无二进制）不可删。
        let err = inv
            .delete(
                &dir.path().join("cores/mihomo/1.19.29"),
                Path::new("/nonexistent/active"),
            )
            .unwrap_err();
        assert!(
            err.to_string().contains("无效的核心二进制路径"),
            "应拒绝目录: {err}"
        );
        assert!(dir.path().join("cores/mihomo/1.19.29").is_dir());
    }
}
