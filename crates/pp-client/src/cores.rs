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
    /// 已完成同版本下载时直接复用；下载后解压、chmod 755 并以 `--version`
    /// 校验输出包含目标版本。
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

        // `--version` 校验输出包含目标版本；失败时清理目录避免残留半成品。
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
    /// 以 `--version` 解析版本号；解析失败记为 `unknown`。
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

/// 运行 `--version` 并拼接 stdout / stderr。
fn binary_output(binary: &Path) -> String {
    Command::new(binary)
        .arg("--version")
        .output()
        .map(|o| {
            format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            )
        })
        .unwrap_or_default()
}

/// 从 `--version` 输出解析版本号。
///
/// sing-box: `sing-box version 1.13.15`
/// mihomo:   `Mihomo Meta v1.19.29 linux/amd64 go1.23.4`
fn parse_version_from_output(core_type: CoreType, output: &str) -> Option<String> {
    let pattern = match core_type {
        CoreType::SingBox => r"sing-box\s+version\s+v?([0-9][0-9A-Za-z.\-]*)",
        CoreType::Mihomo => r"(?i)mihomo[^\n]*?\bv?([0-9][0-9A-Za-z.\-]*)",
    };
    let re = regex::Regex::new(pattern).ok()?;
    re.captures(output)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

/// 校验 `--version` 输出包含目标版本（允许 `v` 前缀，或解析出的版本号相等）。
fn verify_version(binary: &Path, core_type: CoreType, version: &str) -> PanelResult<()> {
    let text = binary_output(binary);
    let parsed = parse_version_from_output(core_type, &text).unwrap_or_default();
    if !version.is_empty()
        && (text.contains(version) || text.contains(&format!("v{version}")) || parsed == version)
    {
        return Ok(());
    }
    Err(PanelError::Core(format!(
        "核心 {core_type} 版本校验失败：请求 {version}，--version 输出：{text}"
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

        let old = std::env::var_os("PATH");
        // Rust 2024 下 std::env 的 set_var/remove_var 标记为 unsafe（并发修改
        // 环境变量是未定义行为），测试进程内单线程调用。
        unsafe {
            std::env::set_var("PATH", dir.path());
        }
        let result = {
            let inv = ClientCoreInventory::new(PathBuf::new());
            inv.detect_system_cores()
        };
        match old {
            Some(v) => unsafe { std::env::set_var("PATH", v) },
            None => unsafe { std::env::remove_var("PATH") },
        }

        let found = result
            .iter()
            .find(|c| c.core_type == CoreType::SingBox && c.path == bin);
        assert!(found.is_some(), "应在 PATH 中发现假 sing-box");
        assert_eq!(found.unwrap().version, "1.19.9");
        assert_eq!(found.unwrap().source, CoreSource::System);
    }

    // ---------- ⑤ active_core：按 config.core_binary 匹配 ----------

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

    // ---------- ⑥ infer_core_type：文件名推断 ----------

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
                CoreType::Mihomo,
                "Mihomo Meta v1.19.29 linux/amd64 go1.23.4"
            ),
            Some("1.19.29".to_string())
        );
    }
}
