//! TUN 提权检测与授权。
//!
//! 桌面客户端启用 TUN 虚拟网卡需要特权（Linux `cap_net_admin` / macOS root
//! + setuid / Windows 管理员令牌）。本模块提供权限检测（[`tun_auth_status`]）与
//!   授权入口（[`authorize_tun`]），供启动流程前置检查与设置页授权按钮调用。
use std::path::Path;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::process::Command;

use pp_common::{PanelError, PanelResult};

/// TUN 提权状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TunAuthStatus {
    /// 已具备 TUN 所需权限，可直接启动。
    Authorized,
    /// 未具备权限，需要用户授权。
    NeedsAuth,
    /// 当前平台不支持自动提权（携带原因）。
    Unsupported(String),
}

impl TunAuthStatus {
    /// 前端字符串表示：`authorized` / `needs_auth` / `unsupported:<reason>`。
    pub fn as_frontend_str(&self) -> String {
        match self {
            TunAuthStatus::Authorized => "authorized".to_string(),
            TunAuthStatus::NeedsAuth => "needs_auth".to_string(),
            TunAuthStatus::Unsupported(reason) => format!("unsupported:{reason}"),
        }
    }
}

/// 检测核心二进制是否已具备 TUN 权限。
///
/// - **Linux**：`getcap <binary>` 输出含 `cap_net_admin` → [`TunAuthStatus::Authorized`]；
///   否则（含 getcap 不可用 / 文件不存在）→ [`TunAuthStatus::NeedsAuth`]。
/// - **macOS**：文件 owner 为 root 且 setuid 位已设置 → `Authorized`；否则 `NeedsAuth`。
/// - **Windows**：简化实现（未内置管理员令牌检测），一律返回 `NeedsAuth`，
///   由 [`authorize_tun`] 提示以管理员身份重启应用。
pub fn tun_auth_status(core_binary: &Path) -> TunAuthStatus {
    platform_tun_auth_status(core_binary)
}

#[cfg(target_os = "linux")]
fn platform_tun_auth_status(core_binary: &Path) -> TunAuthStatus {
    // getcap 不可用 / 文件不存在时保守返回 NeedsAuth。
    let Ok(output) = Command::new("getcap").arg(core_binary).output() else {
        return TunAuthStatus::NeedsAuth;
    };
    if !output.status.success() {
        return TunAuthStatus::NeedsAuth;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    if text.contains("cap_net_admin") {
        TunAuthStatus::Authorized
    } else {
        TunAuthStatus::NeedsAuth
    }
}

#[cfg(target_os = "macos")]
fn platform_tun_auth_status(core_binary: &Path) -> TunAuthStatus {
    use std::os::unix::fs::MetadataExt;
    let Ok(meta) = std::fs::metadata(core_binary) else {
        return TunAuthStatus::NeedsAuth;
    };
    if meta.uid() == 0 && meta.mode() & 0o4000 != 0 {
        TunAuthStatus::Authorized
    } else {
        TunAuthStatus::NeedsAuth
    }
}

#[cfg(target_os = "windows")]
fn platform_tun_auth_status(_core_binary: &Path) -> TunAuthStatus {
    // 简化实现：不做管理员令牌检测，一律提示需要授权（以管理员身份重启）。
    TunAuthStatus::NeedsAuth
}

#[cfg(target_os = "android")]
fn platform_tun_auth_status(_core_binary: &Path) -> TunAuthStatus {
    // Android 的 TUN 通过 VpnService 系统授权，无需（也无法）对核心二进制提权。
    TunAuthStatus::Unsupported("Android 使用 VpnService 授权".to_string())
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows", target_os = "android")))]
fn platform_tun_auth_status(_core_binary: &Path) -> TunAuthStatus {
    TunAuthStatus::Unsupported("当前平台不支持 TUN".to_string())
}

/// Linux 提权命令 argv（纯函数，供测试断言；不执行）。
///
/// `pkexec setcap 'cap_net_admin,cap_net_bind_service=+ep' <binary>`
pub fn linux_authorize_cmdline(core_binary: &Path) -> Vec<String> {
    vec![
        "pkexec".to_string(),
        "setcap".to_string(),
        "cap_net_admin,cap_net_bind_service=+ep".to_string(),
        core_binary.to_string_lossy().into_owned(),
    ]
}

/// macOS 提权命令 argv（纯函数，供测试断言；不执行）。
///
/// `osascript -e 'do shell script "chown root:admin <bin> && chmod u+s <bin>" with administrator privileges'`
pub fn macos_authorize_cmdline(core_binary: &Path) -> Vec<String> {
    vec![
        "osascript".to_string(),
        "-e".to_string(),
        format!(
            "do shell script \"chown root:admin {} && chmod u+s {}\" with administrator privileges",
            core_binary.display(),
            core_binary.display()
        ),
    ]
}

/// 授予核心二进制 TUN 权限；执行成功后再次调用 [`tun_auth_status`] 可确认新状态。
///
/// - **Linux**：`pkexec setcap 'cap_net_admin,cap_net_bind_service=+ep' <binary>`；
///   `pkexec` 不可用时返回错误提示安装 polkit。
/// - **macOS**：`osascript` 以管理员权限执行 `chown root:admin` + `chmod u+s`。
/// - **Windows**：返回 `Unsupported` 错误，提示以管理员身份重启应用。
pub fn authorize_tun(core_binary: &Path) -> PanelResult<()> {
    platform_authorize_tun(core_binary)
}

#[cfg(target_os = "linux")]
fn platform_authorize_tun(core_binary: &Path) -> PanelResult<()> {
    let argv = linux_authorize_cmdline(core_binary);
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]);
    match cmd.status() {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(PanelError::Client(format!(
            "TUN 授权失败：setcap 退出码 {status}，请确认已安装 libcap 工具集（getcap/setcap）"
        ))),
        Err(e) => Err(PanelError::Client(format!(
            "无法执行 pkexec（{e}）：请安装 polkit（如 Debian/Ubuntu: apt install policykit-1）后重试"
        ))),
    }
}

#[cfg(target_os = "macos")]
fn platform_authorize_tun(core_binary: &Path) -> PanelResult<()> {
    let argv = macos_authorize_cmdline(core_binary);
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..]);
    match cmd.status() {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(PanelError::Client(format!(
            "TUN 授权失败：osascript 退出码 {status}"
        ))),
        Err(e) => Err(PanelError::Client(format!(
            "无法执行 osascript（{e}），请确认系统脚本可用"
        ))),
    }
}

#[cfg(target_os = "windows")]
fn platform_authorize_tun(_core_binary: &Path) -> PanelResult<()> {
    Err(PanelError::Client(
        "TUN 授权不支持自动提权：请以管理员身份重启应用后重试".to_string(),
    ))
}

#[cfg(target_os = "android")]
fn platform_authorize_tun(_core_binary: &Path) -> PanelResult<()> {
    Err(PanelError::Client(
        "Android 使用 VpnService 授权 TUN，无需对核心二进制提权".to_string(),
    ))
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows", target_os = "android")))]
fn platform_authorize_tun(_core_binary: &Path) -> PanelResult<()> {
    Err(PanelError::Client(
        "当前平台不支持 TUN 自动提权".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Linux 提权命令构造断言（纯函数，不执行）。
    #[test]
    fn linux_authorize_cmdline_builds_pkexec_setcap() {
        let argv = linux_authorize_cmdline(Path::new("/usr/local/bin/sing-box"));
        assert_eq!(
            argv,
            vec![
                "pkexec".to_string(),
                "setcap".to_string(),
                "cap_net_admin,cap_net_bind_service=+ep".to_string(),
                "/usr/local/bin/sing-box".to_string(),
            ]
        );
    }

    /// macOS 提权命令构造断言（纯函数，不执行）。
    #[test]
    fn macos_authorize_cmdline_builds_osascript() {
        let argv = macos_authorize_cmdline(Path::new("/Applications/pp/sing-box"));
        assert_eq!(argv[0], "osascript");
        assert_eq!(argv[1], "-e");
        assert!(
            argv[2].contains("do shell script \"chown root:admin /Applications/pp/sing-box"),
            "应包含 chown root:admin: {}",
            argv[2]
        );
        assert!(
            argv[2].contains("chmod u+s /Applications/pp/sing-box"),
            "应包含 chmod u+s: {}",
            argv[2]
        );
        assert!(
            argv[2].contains("with administrator privileges"),
            "应请求管理员权限: {}",
            argv[2]
        );
    }

    /// 前端字符串表示。
    #[test]
    fn frontend_str_matches_contract() {
        assert_eq!(TunAuthStatus::Authorized.as_frontend_str(), "authorized");
        assert_eq!(TunAuthStatus::NeedsAuth.as_frontend_str(), "needs_auth");
        assert_eq!(
            TunAuthStatus::Unsupported("no tun".to_string()).as_frontend_str(),
            "unsupported:no tun"
        );
    }

    /// 未授权二进制 → NeedsAuth（Linux：getcap 无可信输出；macOS：owner 非 root）。
    #[cfg(target_os = "linux")]
    #[test]
    fn tun_auth_status_unauthorized_binary_reports_needs_auth() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("fake-core");
        std::fs::write(&bin, b"#!/bin/sh\necho fake\n").unwrap();
        assert_eq!(tun_auth_status(&bin), TunAuthStatus::NeedsAuth);
    }

    /// setcap 授权后 → Authorized（状态流转）。getcap/setcap 不可用或非 root
    /// 时 setcap 失败，跳过该断言。
    #[cfg(target_os = "linux")]
    #[test]
    fn tun_auth_status_authorized_after_setcap_when_available() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("fake-core");
        std::fs::write(&bin, b"#!/bin/sh\necho fake\n").unwrap();
        let setcap_ok = Command::new("setcap")
            .args(["cap_net_admin,cap_net_bind_service=+ep"])
            .arg(&bin)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !setcap_ok {
            eprintln!("setcap 不可用（需要 root），跳过 Authorized 状态流转断言");
            return;
        }
        assert_eq!(tun_auth_status(&bin), TunAuthStatus::Authorized);
    }
}
