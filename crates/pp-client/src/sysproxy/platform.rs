//! 平台系统代理的命令构造与执行（自 `sysproxy.rs` 拆出，控制单文件规模）。

use std::net::SocketAddr;
use std::process::Command;

/// 一条待执行的系统命令描述（仅描述，不执行）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    /// 可执行文件。
    pub program: String,
    /// 参数列表。
    pub args: Vec<String>,
}

impl CommandSpec {
    fn new(program: &str, args: &[&str]) -> Self {
        Self {
            program: program.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// 构造真实的 [`Command`]（供执行路径使用）。Windows 下统一施加
    /// CREATE_NO_WINDOW（reg 等子进程否则每次执行都弹 cmd 窗口）。
    pub fn to_command(&self) -> Command {
        let mut cmd = Command::new(&self.program);
        cmd.args(&self.args);
        pp_common::no_window(&mut cmd);
        cmd
    }
}

/// 目标平台。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOs {
    /// macOS（`networksetup`）。
    MacOs,
    /// Windows（`reg`）。
    Windows,
    /// Linux（`gsettings`，GNOME）。
    Linux,
    /// Android（系统代理由 VpnService 控制，不支持 gsettings/reg 等桌面方案）。
    Android,
}

impl TargetOs {
    /// 当前编译目标平台。
    pub fn current() -> Self {
        #[cfg(target_os = "macos")]
        {
            Self::MacOs
        }
        #[cfg(target_os = "windows")]
        {
            Self::Windows
        }
        #[cfg(target_os = "android")]
        {
            Self::Android
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "android")))]
        {
            Self::Linux
        }
    }
}

/// 基于目标平台执行系统命令的真实实现。
#[derive(Debug, Clone)]
pub struct PlatformSystemProxy {
    /// 目标平台。
    pub os: TargetOs,
    /// macOS 网络接口名（Windows/Linux 忽略）。
    pub interface: String,
}

impl Default for PlatformSystemProxy {
    fn default() -> Self {
        Self {
            os: TargetOs::current(),
            interface: "Wi-Fi".to_string(),
        }
    }
}

impl PlatformSystemProxy {
    /// 指定平台的实例（测试用）。
    pub fn with_os(os: TargetOs) -> Self {
        Self {
            os,
            interface: "Wi-Fi".to_string(),
        }
    }

    pub(crate) const REG_KEY: &'static str =
        r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings";

    /// 纯函数：构造启用命令（不执行）。
    pub fn build_commands(&self, addr: SocketAddr) -> Vec<CommandSpec> {
        let host = addr.ip().to_string();
        let port = addr.port().to_string();
        let iface = self.interface.as_str();
        match self.os {
            TargetOs::MacOs => vec![
                CommandSpec::new("networksetup", &["-setwebproxy", iface, &host, &port]),
                CommandSpec::new("networksetup", &["-setsecurewebproxy", iface, &host, &port]),
                CommandSpec::new("networksetup", &["-setwebproxystate", iface, "on"]),
                CommandSpec::new("networksetup", &["-setsecurewebproxystate", iface, "on"]),
            ],
            TargetOs::Windows => vec![
                CommandSpec::new(
                    "reg",
                    &[
                        "add",
                        Self::REG_KEY,
                        "/v",
                        "ProxyEnable",
                        "/t",
                        "REG_DWORD",
                        "/d",
                        "1",
                        "/f",
                    ],
                ),
                CommandSpec::new(
                    "reg",
                    &[
                        "add",
                        Self::REG_KEY,
                        "/v",
                        "ProxyServer",
                        "/t",
                        "REG_SZ",
                        "/d",
                        &format!("{host}:{port}"),
                        "/f",
                    ],
                ),
            ],
            TargetOs::Linux => vec![
                CommandSpec::new(
                    "gsettings",
                    &["set", "org.gnome.system.proxy", "mode", "manual"],
                ),
                CommandSpec::new(
                    "gsettings",
                    &["set", "org.gnome.system.proxy.http", "host", &host],
                ),
                CommandSpec::new(
                    "gsettings",
                    &["set", "org.gnome.system.proxy.http", "port", &port],
                ),
                CommandSpec::new(
                    "gsettings",
                    &["set", "org.gnome.system.proxy.https", "host", &host],
                ),
                CommandSpec::new(
                    "gsettings",
                    &["set", "org.gnome.system.proxy.https", "port", &port],
                ),
            ],
            // Android 系统代理由 VpnService 控制，无桌面命令可构造；运行期
            // `SystemProxy` 实现已对该平台直接返回错误，不会走到这里。
            TargetOs::Android => vec![],
        }
    }

    /// 纯函数：构造禁用命令（不执行）。
    pub fn build_disable_commands(&self) -> Vec<CommandSpec> {
        let iface = self.interface.as_str();
        match self.os {
            TargetOs::MacOs => vec![
                CommandSpec::new("networksetup", &["-setwebproxystate", iface, "off"]),
                CommandSpec::new("networksetup", &["-setsecurewebproxystate", iface, "off"]),
            ],
            TargetOs::Windows => vec![CommandSpec::new(
                "reg",
                &[
                    "add",
                    Self::REG_KEY,
                    "/v",
                    "ProxyEnable",
                    "/t",
                    "REG_DWORD",
                    "/d",
                    "0",
                    "/f",
                ],
            )],
            TargetOs::Linux => vec![CommandSpec::new(
                "gsettings",
                &["set", "org.gnome.system.proxy", "mode", "none"],
            )],
            // Android 无桌面禁用命令（见 [`TargetOs::Android`]）。
            TargetOs::Android => vec![],
        }
    }

    /// 逐条执行命令，任一条失败即返回错误（`gsettings` 缺失时给出提示）。
    pub(crate) fn run_specs(&self, specs: &[CommandSpec]) -> Result<(), std::io::Error> {
        for spec in specs {
            let output = spec.to_command().output().map_err(|e| {
                if spec.program == "gsettings" {
                    std::io::Error::other(format!(
                        "未找到 gsettings：{e}（请安装 GNOME gsettings-tools 或改用其他代理方案）"
                    ))
                } else {
                    std::io::Error::other(format!("执行系统代理命令 {} 失败：{e}", spec.program))
                }
            })?;
            if !output.status.success() {
                return Err(std::io::Error::other(format!(
                    "系统代理命令 {} {} 执行失败（退出码 {:?}）",
                    spec.program,
                    spec.args.join(" "),
                    output.status.code()
                )));
            }
        }
        Ok(())
    }
}
