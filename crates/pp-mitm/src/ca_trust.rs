//! MITM CA 系统信任库安装与信任状态检测。
//!
//! 平台三分（其余平台编译为「不支持 / 未知」兜底，保证任意 host 可编译）：
//!
//! - **Windows**：`certutil -user -addstore Root`（CurrentUser 存储，免 UAC）；
//!   状态经 `certutil -user -store Root` 输出匹配 CA 主体名。
//! - **macOS**：`osascript` 以管理员权限执行 `security add-trusted-cert` 写入
//!   系统钥匙串；状态经 `security verify-cert -c <ca.crt>` 退出码判定。
//! - **Linux**：`pkexec` 将 CA 安装到 `/usr/local/share/ca-certificates/` 并
//!   `update-ca-certificates`；状态比对锚点目录中证书的 SHA-256 指纹。

use std::path::Path;

use pp_common::error::{PanelError, PanelResult};
use serde::Serialize;

/// Linux 系统锚点中的固定安装文件名。
const LINUX_ANCHOR_NAME: &str = "proxypanel-mitm.crt";

/// Linux 锚点候选目录（Debian/Ubuntu 系与 Arch/Fedora p11-kit 系）。
const LINUX_ANCHOR_DIRS: [&str; 2] = [
    "/usr/local/share/ca-certificates",
    "/etc/ca-certificates/trust-source/anchors",
];

/// 本 CA 的主体 CN（与 [`crate::ca`] 生成的 CN 一致），用于 Windows certutil 输出匹配。
pub const CA_SUBJECT_CN: &str = "ProxyPanel MITM CA";

/// CA 系统信任状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaTrustStatus {
    /// 已安装到系统信任库且与本机 CA 匹配。
    Trusted,
    /// 系统信任库中未找到该 CA（或存在同名但指纹不一致的旧 CA）。
    NotTrusted,
    /// 无法判定（检测命令不可用 / 读取失败 / 平台不支持）。
    Unknown,
}

/// CA 信任状态视图（Tauri 命令返回）。
#[derive(Debug, Clone, Serialize)]
pub struct CaTrustView {
    pub status: CaTrustStatus,
    /// 状态说明（面向用户的一行描述）。
    pub detail: String,
}

impl CaTrustView {
    fn trusted(detail: impl Into<String>) -> Self {
        Self {
            status: CaTrustStatus::Trusted,
            detail: detail.into(),
        }
    }

    fn not_trusted(detail: impl Into<String>) -> Self {
        Self {
            status: CaTrustStatus::NotTrusted,
            detail: detail.into(),
        }
    }

    fn unknown(detail: impl Into<String>) -> Self {
        Self {
            status: CaTrustStatus::Unknown,
            detail: detail.into(),
        }
    }
}

/// 将 CA 安装到系统信任库。
///
/// Windows 写 CurrentUser 存储（不弹 UAC）；macOS / Linux 会弹系统授权框。
pub fn install_ca(ca_path: &Path) -> PanelResult<()> {
    platform_install_ca(ca_path)
}

/// 检测 CA 是否已被系统信任库信任。
///
/// 检测失败（命令缺失、读取失败等）返回 [`CaTrustStatus::Unknown`] 而非 `Err`，
/// 前端据此展示灰色「未知」而非报错。
pub fn ca_trust_status(ca_path: &Path) -> CaTrustView {
    platform_ca_trust_status(ca_path)
}

// ---------------------------------------------------------------------------
// 纯函数（参数化，供单测；不执行系统命令）
// ---------------------------------------------------------------------------

/// PEM → DER（取首个 CERTIFICATE 块）。
pub fn pem_to_der(pem: &str) -> PanelResult<Vec<u8>> {
    let mut reader = pem.as_bytes();
    match rustls_pemfile::certs(&mut reader).next() {
        Some(item) => {
            let der = item.map_err(|e| PanelError::Mitm(format!("解析 CA PEM 失败: {e}")))?;
            Ok(der.to_vec())
        }
        None => Err(PanelError::Mitm("PEM 中未找到证书块".to_string())),
    }
}

/// 字节的 SHA-256 指纹（小写 hex，无分隔符）。
pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// `certutil -user -store Root` 输出中是否包含目标主体名。
///
/// certutil 的 Subject 行为 `CN=ProxyPanel MITM CA`；忽略大小写与全部空白后
/// 匹配，以容忍本地化输出、换行与缩进差异（中文系统 OEM 编码不影响 ASCII
/// 主体名的匹配）。
pub fn certutil_output_contains(output: &str, subject: &str) -> bool {
    fn normalize(s: &str) -> String {
        s.chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_lowercase()
    }
    normalize(output).contains(&normalize(subject))
}

/// Linux 提权安装的 shell 脚本（pkexec `sh -c` 的第三个参数；纯构造函数）。
pub fn linux_install_script(ca_path: &Path) -> String {
    format!(
        "install -m 0644 {} /usr/local/share/ca-certificates/{LINUX_ANCHOR_NAME} && update-ca-certificates",
        shell_quote(&ca_path.to_string_lossy())
    )
}

/// macOS 提权安装的 AppleScript（osascript `-e` 参数；纯构造函数）。
pub fn macos_install_applescript(ca_path: &Path) -> String {
    let path = ca_path
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    format!(
        "do shell script \"security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain \\\"{path}\\\"\" with administrator privileges"
    )
}

/// Windows 安装命令 argv（纯构造函数）。
pub fn windows_install_cmdline(ca_path: &Path) -> Vec<String> {
    vec![
        "certutil".to_string(),
        "-user".to_string(),
        "-addstore".to_string(),
        "Root".to_string(),
        ca_path.to_string_lossy().into_owned(),
    ]
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

// ---------------------------------------------------------------------------
// Windows
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn platform_install_ca(ca_path: &Path) -> PanelResult<()> {
    let argv = windows_install_cmdline(ca_path);
    let output = std::process::Command::new(&argv[0])
        .args(&argv[1..])
        .output()
        .map_err(|e| PanelError::Mitm(format!("无法执行 certutil: {e}")))?;
    if output.status.success() {
        return Ok(());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(PanelError::Mitm(format!(
        "certutil -addstore 失败（退出码 {}）: {}{}",
        output.status,
        stdout.trim(),
        stderr.trim()
    )))
}

#[cfg(target_os = "windows")]
fn platform_ca_trust_status(_ca_path: &Path) -> CaTrustView {
    let output = match std::process::Command::new("certutil")
        .args(["-user", "-store", "Root"])
        .output()
    {
        Ok(o) => o,
        Err(e) => return CaTrustView::unknown(format!("无法执行 certutil: {e}")),
    };
    if !output.status.success() {
        return CaTrustView::unknown(format!("certutil -store 退出码 {}", output.status));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    if certutil_output_contains(&text, CA_SUBJECT_CN) {
        CaTrustView::trusted("已存在于当前用户 Root 信任存储")
    } else {
        CaTrustView::not_trusted("当前用户 Root 信任存储中未找到该 CA")
    }
}

// ---------------------------------------------------------------------------
// macOS
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn platform_install_ca(ca_path: &Path) -> PanelResult<()> {
    let script = macos_install_applescript(ca_path);
    match std::process::Command::new("osascript")
        .args(["-e", &script])
        .status()
    {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(PanelError::Mitm(format!(
            "安装 CA 失败：osascript 退出码 {status}（若取消了授权弹窗请重试）"
        ))),
        Err(e) => Err(PanelError::Mitm(format!("无法执行 osascript: {e}"))),
    }
}

#[cfg(target_os = "macos")]
fn platform_ca_trust_status(ca_path: &Path) -> CaTrustView {
    match std::process::Command::new("security")
        .args(["verify-cert", "-c"])
        .arg(ca_path)
        .status()
    {
        Ok(status) if status.success() => CaTrustView::trusted("系统钥匙串已信任该 CA"),
        Ok(status) => CaTrustView::not_trusted(format!(
            "security verify-cert 退出码 {status}：CA 未被系统信任"
        )),
        Err(e) => CaTrustView::unknown(format!("无法执行 security: {e}")),
    }
}

// ---------------------------------------------------------------------------
// Linux
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
fn platform_install_ca(ca_path: &Path) -> PanelResult<()> {
    let script = linux_install_script(ca_path);
    match std::process::Command::new("pkexec")
        .args(["sh", "-c", &script])
        .status()
    {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(PanelError::Mitm(format!(
            "安装 CA 失败：pkexec 退出码 {status}（需要 update-ca-certificates 与 polkit 授权）"
        ))),
        Err(e) => Err(PanelError::Mitm(format!(
            "无法执行 pkexec（{e}）：请安装 polkit（如 apt install policykit-1）后重试"
        ))),
    }
}

#[cfg(target_os = "linux")]
fn platform_ca_trust_status(ca_path: &Path) -> CaTrustView {
    let ours_pem = match std::fs::read_to_string(ca_path) {
        Ok(p) => p,
        Err(e) => return CaTrustView::unknown(format!("读取本机 CA 失败: {e}")),
    };
    let ours = match pem_to_der(&ours_pem) {
        Ok(d) => d,
        Err(e) => return CaTrustView::unknown(format!("解析本机 CA 失败: {e}")),
    };
    let ours_fp = sha256_hex(&ours);
    let mut found_anchor = false;
    for dir in LINUX_ANCHOR_DIRS {
        let anchor = Path::new(dir).join(LINUX_ANCHOR_NAME);
        match std::fs::read_to_string(&anchor) {
            Ok(pem) => {
                found_anchor = true;
                match pem_to_der(&pem) {
                    Ok(der) if sha256_hex(&der) == ours_fp => {
                        return CaTrustView::trusted(format!(
                            "系统锚点 {} 与本机 CA 指纹一致",
                            anchor.display()
                        ));
                    }
                    // 同名锚点但指纹不一致（旧 CA）：继续检查其他锚点目录。
                    Ok(_) => {}
                    Err(e) => {
                        return CaTrustView::unknown(format!(
                            "解析 {} 失败: {e}",
                            anchor.display()
                        ));
                    }
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return CaTrustView::unknown(format!("读取 {} 失败: {e}", anchor.display())),
        }
    }
    if found_anchor {
        CaTrustView::not_trusted("系统锚点存在同名 CA 但指纹不一致（可能为旧 CA，请重新安装）")
    } else {
        CaTrustView::not_trusted("系统锚点中未找到该 CA")
    }
}

// ---------------------------------------------------------------------------
// 兜底（其他平台：可编译，安装报不支持、状态报未知）
// ---------------------------------------------------------------------------

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn platform_install_ca(_ca_path: &Path) -> PanelResult<()> {
    Err(PanelError::Mitm(
        "当前平台不支持自动安装 CA 到系统信任库".to_string(),
    ))
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn platform_ca_trust_status(_ca_path: &Path) -> CaTrustView {
    CaTrustView::unknown("当前平台不支持信任状态检测")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CaStore;

    #[test]
    fn pem_to_der_extracts_first_cert_block() {
        let dir = tempfile::tempdir().unwrap();
        let material = crate::FileCaStore::new(dir.path())
            .load_or_generate()
            .unwrap();
        let der = pem_to_der(&material.cert_pem).unwrap();
        assert!(!der.is_empty());
        assert_eq!(der[0], 0x30, "DER 应以 SEQUENCE 开头");
    }

    #[test]
    fn pem_to_der_rejects_pem_without_cert() {
        let err = pem_to_der("-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----\n");
        assert!(err.is_err());
    }

    #[test]
    fn sha256_hex_matches_known_vector() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn sha256_fingerprint_stable_for_same_der() {
        let dir = tempfile::tempdir().unwrap();
        let material = crate::FileCaStore::new(dir.path())
            .load_or_generate()
            .unwrap();
        let a = pem_to_der(&material.cert_pem).unwrap();
        let b = pem_to_der(&material.cert_pem).unwrap();
        assert_eq!(sha256_hex(&a), sha256_hex(&b));
    }

    #[test]
    fn certutil_output_matches_subject_ignoring_case_and_whitespace() {
        let output = "\
================ 证书 0 ================
序列号: 1a2b3c
颁发者: CN=ProxyPanel MITM CA
 NotBefore: 2026/1/1
 NotAfter: 2036/1/1
使用者: CN=ProxyPanel MITM CA
证书哈希(sha1): aa bb cc
";
        assert!(certutil_output_contains(output, CA_SUBJECT_CN));
        assert!(!certutil_output_contains(output, "Some Other CA"));
        // 大小写与空白不敏感。
        assert!(certutil_output_contains(
            "subject: cn=proxypanel  mitm\tca",
            CA_SUBJECT_CN
        ));
    }

    #[test]
    fn linux_install_script_quotes_path_and_updates_store() {
        let script = linux_install_script(Path::new("/home/user/my dir/ca.crt"));
        assert!(
            script.contains(
                "install -m 0644 '/home/user/my dir/ca.crt' /usr/local/share/ca-certificates/proxypanel-mitm.crt"
            ),
            "应引用并安装到锚点目录: {script}"
        );
        assert!(script.ends_with("&& update-ca-certificates"), "{script}");
    }

    #[test]
    fn linux_install_script_escapes_single_quotes() {
        let script = linux_install_script(Path::new("/tmp/it's/ca.crt"));
        assert!(script.contains("/tmp/it'\\''s/ca.crt"), "{script}");
    }

    #[test]
    fn macos_install_applescript_embeds_path_and_admin_flag() {
        let script = macos_install_applescript(Path::new("/Users/u/ca.crt"));
        assert!(
            script.contains("security add-trusted-cert -d -r trustRoot"),
            "{script}"
        );
        assert!(
            script.contains("/Library/Keychains/System.keychain"),
            "{script}"
        );
        assert!(script.contains("\\\"/Users/u/ca.crt\\\""), "{script}");
        assert!(script.contains("with administrator privileges"), "{script}");
    }

    #[test]
    fn windows_install_cmdline_targets_current_user_root_store() {
        let argv = windows_install_cmdline(Path::new("C:\\Users\\u\\ca.crt"));
        assert_eq!(
            argv,
            vec![
                "certutil".to_string(),
                "-user".to_string(),
                "-addstore".to_string(),
                "Root".to_string(),
                "C:\\Users\\u\\ca.crt".to_string(),
            ]
        );
    }

    #[test]
    fn trust_status_serializes_to_frontend_contract() {
        let view = CaTrustView::trusted("ok");
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(json["status"], "trusted");
        let view = CaTrustView::not_trusted("no");
        assert_eq!(
            serde_json::to_value(&view).unwrap()["status"],
            "not_trusted"
        );
        let view = CaTrustView::unknown("?");
        assert_eq!(serde_json::to_value(&view).unwrap()["status"], "unknown");
    }

    /// Linux 上本机（未安装 CA 的开发环境）检测应给出 NotTrusted 或 Unknown，
    /// 绝不 panic / Err。
    #[cfg(target_os = "linux")]
    #[test]
    fn linux_status_never_panics_for_fresh_ca() {
        let dir = tempfile::tempdir().unwrap();
        let material = crate::FileCaStore::new(dir.path())
            .load_or_generate()
            .unwrap();
        std::fs::write(dir.path().join("ca.crt"), &material.cert_pem).unwrap();
        let view = ca_trust_status(&dir.path().join("ca.crt"));
        assert!(matches!(
            view.status,
            CaTrustStatus::NotTrusted | CaTrustStatus::Unknown | CaTrustStatus::Trusted
        ));
    }
}
