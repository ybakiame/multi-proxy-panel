//! 系统代理控制。
//!
//! 提供跨平台的系统 HTTP/HTTPS 代理开关：
//! - macOS 使用 `networksetup`（作用于网络接口，默认 "Wi-Fi"）
//! - Windows 使用 `reg add` 写 `Internet Settings`
//! - Linux 使用 `gsettings`（GNOME），未安装 `gsettings` 时报错提示
//!
//! 命令构造为纯函数 [`PlatformSystemProxy::build_commands`]，便于单测断言
//! 而不实际修改系统。运行期开关通过 [`SystemProxy`] trait 抽象，测试可注入
//! [`MockSystemProxy`]。

use std::net::SocketAddr;
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use pp_common::PanelResult;

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

    /// 构造真实的 [`Command`]（供执行路径使用）。
    pub fn to_command(&self) -> Command {
        let mut cmd = Command::new(&self.program);
        cmd.args(&self.args);
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
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            Self::Linux
        }
    }
}

/// 系统代理控制接口。
#[async_trait]
pub trait SystemProxy: Send + Sync {
    /// 启用系统 HTTP/HTTPS 代理（同一地址）。
    async fn enable(&self, http_proxy_addr: SocketAddr) -> PanelResult<()>;
    /// 禁用系统代理。
    async fn disable(&self) -> PanelResult<()>;
    /// 系统代理当前是否处于启用状态。
    async fn is_enabled(&self) -> bool;
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

    const REG_KEY: &'static str =
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
        }
    }

    /// 逐条执行命令，任一条失败即返回错误（`gsettings` 缺失时给出提示）。
    fn run_specs(&self, specs: &[CommandSpec]) -> Result<(), std::io::Error> {
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

#[async_trait]
impl SystemProxy for PlatformSystemProxy {
    async fn enable(&self, http_proxy_addr: SocketAddr) -> PanelResult<()> {
        let specs = self.build_commands(http_proxy_addr);
        Ok(self.run_specs(&specs)?)
    }

    async fn disable(&self) -> PanelResult<()> {
        let specs = self.build_disable_commands();
        Ok(self.run_specs(&specs)?)
    }

    async fn is_enabled(&self) -> bool {
        match self.os {
            TargetOs::Linux => {
                let out = Command::new("gsettings")
                    .args(["get", "org.gnome.system.proxy", "mode"])
                    .output()
                    .ok();
                out.map(|o| String::from_utf8_lossy(&o.stdout).trim() == "'manual'")
                    .unwrap_or(false)
            }
            TargetOs::MacOs => {
                let out = Command::new("networksetup")
                    .args(["-getwebproxystate", self.interface.as_str()])
                    .output()
                    .ok();
                out.map(|o| String::from_utf8_lossy(&o.stdout).contains("Enabled"))
                    .unwrap_or(false)
            }
            TargetOs::Windows => {
                let out = Command::new("reg")
                    .args(["query", Self::REG_KEY, "/v", "ProxyEnable"])
                    .output()
                    .ok();
                out.map(|o| String::from_utf8_lossy(&o.stdout).contains("0x1"))
                    .unwrap_or(false)
            }
        }
    }
}

/// 一次系统代理调用记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SysProxyCall {
    /// 启用，记录目标地址。
    Enable(SocketAddr),
    /// 禁用。
    Disable,
}

/// 记录调用的测试用实现。
#[derive(Default)]
pub struct MockSystemProxy {
    calls: Mutex<Vec<SysProxyCall>>,
    enabled: AtomicBool,
}

impl MockSystemProxy {
    /// 空记录实例。
    pub fn new() -> Self {
        Self::default()
    }

    /// 已记录的调用序列。
    pub fn calls(&self) -> Vec<SysProxyCall> {
        self.calls.lock().expect("sysproxy mock 锁污染").clone()
    }

    /// `enable` 被调用的次数。
    pub fn enable_count(&self) -> usize {
        self.calls()
            .iter()
            .filter(|c| matches!(c, SysProxyCall::Enable(_)))
            .count()
    }
}

#[async_trait]
impl SystemProxy for MockSystemProxy {
    async fn enable(&self, http_proxy_addr: SocketAddr) -> PanelResult<()> {
        self.calls
            .lock()
            .expect("sysproxy mock 锁污染")
            .push(SysProxyCall::Enable(http_proxy_addr));
        self.enabled.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn disable(&self) -> PanelResult<()> {
        self.calls
            .lock()
            .expect("sysproxy mock 锁污染")
            .push(SysProxyCall::Disable);
        self.enabled.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    fn addr() -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 18080)
    }

    #[test]
    fn macos_build_commands() {
        let p = PlatformSystemProxy::with_os(TargetOs::MacOs);
        let cmds = p.build_commands(addr());
        assert_eq!(cmds.len(), 4);
        assert_eq!(cmds[0].program, "networksetup");
        assert_eq!(
            cmds[0].args,
            vec!["-setwebproxy", "Wi-Fi", "127.0.0.1", "18080"]
        );
        assert_eq!(
            cmds[1].args,
            vec!["-setsecurewebproxy", "Wi-Fi", "127.0.0.1", "18080"]
        );
        assert_eq!(cmds[2].args, vec!["-setwebproxystate", "Wi-Fi", "on"]);
        assert_eq!(cmds[3].args, vec!["-setsecurewebproxystate", "Wi-Fi", "on"]);
    }

    #[test]
    fn windows_build_commands() {
        let p = PlatformSystemProxy::with_os(TargetOs::Windows);
        let cmds = p.build_commands(addr());
        assert_eq!(cmds.len(), 2);
        assert_eq!(
            cmds[0].args,
            vec![
                "add",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings",
                "/v",
                "ProxyEnable",
                "/t",
                "REG_DWORD",
                "/d",
                "1",
                "/f",
            ]
        );
        assert_eq!(
            cmds[1].args,
            vec![
                "add",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings",
                "/v",
                "ProxyServer",
                "/t",
                "REG_SZ",
                "/d",
                "127.0.0.1:18080",
                "/f",
            ]
        );
    }

    #[test]
    fn linux_build_commands() {
        let p = PlatformSystemProxy::with_os(TargetOs::Linux);
        let cmds = p.build_commands(addr());
        assert_eq!(cmds.len(), 5);
        assert_eq!(
            cmds[0].args,
            vec!["set", "org.gnome.system.proxy", "mode", "manual"]
        );
        assert_eq!(
            cmds[1].args,
            vec!["set", "org.gnome.system.proxy.http", "host", "127.0.0.1"]
        );
        assert_eq!(
            cmds[2].args,
            vec!["set", "org.gnome.system.proxy.http", "port", "18080"]
        );
        assert_eq!(
            cmds[3].args,
            vec!["set", "org.gnome.system.proxy.https", "host", "127.0.0.1"]
        );
        assert_eq!(
            cmds[4].args,
            vec!["set", "org.gnome.system.proxy.https", "port", "18080"]
        );
    }

    #[test]
    fn disable_commands() {
        let mac = PlatformSystemProxy::with_os(TargetOs::MacOs);
        assert_eq!(
            mac.build_disable_commands()[0].args,
            vec!["-setwebproxystate", "Wi-Fi", "off"]
        );

        let win = PlatformSystemProxy::with_os(TargetOs::Windows);
        let win_cmd = &win.build_disable_commands()[0];
        assert_eq!(
            win_cmd.args,
            vec![
                "add",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings",
                "/v",
                "ProxyEnable",
                "/t",
                "REG_DWORD",
                "/d",
                "0",
                "/f",
            ]
        );

        let lin = PlatformSystemProxy::with_os(TargetOs::Linux);
        assert_eq!(
            lin.build_disable_commands()[0].args,
            vec!["set", "org.gnome.system.proxy", "mode", "none"]
        );
    }

    #[tokio::test]
    async fn mock_records_calls() {
        let m = MockSystemProxy::new();
        assert!(!m.is_enabled().await);
        m.enable(addr()).await.unwrap();
        assert!(m.is_enabled().await);
        assert_eq!(m.calls(), vec![SysProxyCall::Enable(addr())]);
        m.disable().await.unwrap();
        assert_eq!(
            m.calls(),
            vec![SysProxyCall::Enable(addr()), SysProxyCall::Disable]
        );
        assert!(!m.is_enabled().await);
    }
}
