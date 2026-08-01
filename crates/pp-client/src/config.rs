//! 客户端核心配置（桌面客户端核心库）。

use std::path::{Path, PathBuf};

use pp_common::{CoreType, PanelResult};
use serde::{Deserialize, Serialize};

/// MITM 代理的客户端视图配置。
///
/// 对应 `pp_mitm::MitmConfig`，仅保留桌面客户端需要持久化的字段。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MitmClientConfig {
    /// 存放 MITM CA（`ca.crt` / `ca.key`）的目录，默认 `data_dir/certs`。
    pub ca_dir: PathBuf,
    /// 需要拦截的主机名列表，空列表表示全拦截。
    pub hostnames: Vec<String>,
    /// 脚本钩子使用的脚本方言。
    pub script_dialect: pp_script::ScriptDialect,
}

impl Default for MitmClientConfig {
    fn default() -> Self {
        Self {
            ca_dir: PathBuf::new(),
            hostnames: Vec::new(),
            script_dialect: pp_script::ScriptDialect::Surge,
        }
    }
}

/// 桌面客户端核心配置。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientConfig {
    /// 数据目录（配置、证书、核心二进制等统一存放于此）。
    pub data_dir: PathBuf,
    /// Hub 地址，如 `http://127.0.0.1:50052`。
    pub hub_url: String,
    /// 订阅 token。
    pub sub_token: String,
    /// 使用的核心类型。
    pub core_type: CoreType,
    /// 核心二进制路径。
    pub core_binary: PathBuf,
    /// 本地 mixed 入站端口。
    pub mixed_port: u16,
    /// 是否启用 MITM。
    pub mitm_enabled: bool,
    /// MITM 配置（客户端视图）。
    pub mitm: MitmClientConfig,
    /// 是否启用系统代理。
    pub system_proxy_enabled: bool,
    /// 是否启用 TUN 虚拟网卡（需要 root/管理员权限）。
    pub tun_enabled: bool,
    /// TUN 协议栈：`gvisor` / `system` / `mixed`。
    pub tun_stack: String,
    /// TUN 自动路由（默认开启）。
    pub tun_auto_route: bool,
    /// 是否启用 Clash 面板 API（RESTful 控制接口）。
    pub clash_api_enabled: bool,
    /// Clash 面板 API 监听端口。
    pub clash_api_port: u16,
    /// Clash 面板 API 密钥（空串 = 不鉴权，合成配置时省略该字段）。
    pub clash_api_secret: String,
    /// Clash 面板 UI 选择：`yacd` / `zashboard` / `metacubexd`（默认 `zashboard`）。
    ///
    /// 未知值在配置合成（`core_config::apply_panel_features*`）时回退为 `zashboard`。
    pub clash_api_ui: String,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            data_dir: PathBuf::new(),
            hub_url: String::new(),
            sub_token: String::new(),
            core_type: CoreType::SingBox,
            core_binary: PathBuf::new(),
            mixed_port: 17890,
            mitm_enabled: true,
            mitm: MitmClientConfig::default(),
            system_proxy_enabled: false,
            tun_enabled: false,
            tun_stack: "mixed".to_string(),
            tun_auto_route: true,
            clash_api_enabled: false,
            clash_api_port: 9090,
            clash_api_secret: String::new(),
            clash_api_ui: "zashboard".to_string(),
        }
    }
}

impl ClientConfig {
    /// 基于数据目录构造配置，`ca_dir` 默认指向 `data_dir/certs`。
    pub fn new(
        data_dir: PathBuf,
        hub_url: impl Into<String>,
        sub_token: impl Into<String>,
        core_type: CoreType,
        core_binary: PathBuf,
    ) -> Self {
        let mut cfg = Self {
            data_dir,
            hub_url: hub_url.into(),
            sub_token: sub_token.into(),
            core_type,
            core_binary,
            ..Self::default()
        };
        cfg.mitm.ca_dir = cfg.data_dir.join("certs");
        cfg
    }

    /// 配置文件路径：`data_dir/client.json`。
    pub fn config_file(&self) -> PathBuf {
        self.data_dir.join("client.json")
    }

    /// 从 `data_dir/client.json` 加载配置。
    pub fn load(data_dir: &Path) -> PanelResult<Self> {
        let text = std::fs::read_to_string(data_dir.join("client.json"))?;
        Ok(serde_json::from_str(&text)?)
    }

    /// 将配置保存到 `data_dir/client.json`。
    pub fn save(&self) -> PanelResult<()> {
        let path = self.config_file();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, text)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_sane_defaults() {
        let cfg = ClientConfig::default();
        assert_eq!(cfg.mixed_port, 17890);
        assert!(cfg.mitm_enabled);
        assert!(!cfg.system_proxy_enabled);
        assert!(cfg.mitm.hostnames.is_empty());
        assert!(matches!(
            cfg.mitm.script_dialect,
            pp_script::ScriptDialect::Surge
        ));
        // TUN / Clash 面板配置默认关闭，默认值向后兼容。
        assert!(!cfg.tun_enabled);
        assert_eq!(cfg.tun_stack, "mixed");
        assert!(cfg.tun_auto_route);
        assert!(!cfg.clash_api_enabled);
        assert_eq!(cfg.clash_api_port, 9090);
        assert!(cfg.clash_api_secret.is_empty());
        assert_eq!(cfg.clash_api_ui, "zashboard");
    }

    #[test]
    fn new_config_wires_ca_dir_to_data_dir() {
        let cfg = ClientConfig::new(
            PathBuf::from("/tmp/pp-client-test"),
            "http://127.0.0.1:50052",
            "abc123",
            CoreType::SingBox,
            PathBuf::from("/usr/local/bin/sing-box"),
        );
        assert_eq!(cfg.mitm.ca_dir, PathBuf::from("/tmp/pp-client-test/certs"));
    }

    #[test]
    fn serde_roundtrip() {
        let mut cfg = ClientConfig::new(
            PathBuf::from("/tmp/pp-client-test"),
            "http://127.0.0.1:50052",
            "abc123",
            CoreType::Mihomo,
            PathBuf::from("/usr/local/bin/mihomo"),
        );
        cfg.tun_enabled = true;
        cfg.tun_stack = "system".to_string();
        cfg.tun_auto_route = false;
        cfg.clash_api_enabled = true;
        cfg.clash_api_port = 9091;
        cfg.clash_api_secret = "sekret".to_string();
        cfg.clash_api_ui = "metacubexd".to_string();
        let json = serde_json::to_string(&cfg).unwrap();
        let back: ClientConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg, back);
    }

    #[test]
    fn serde_missing_new_fields_defaults() {
        // 旧版 client.json 缺失 TUN / Clash 字段时按默认值解析（serde default 全兼容）。
        let json = r#"{
            "data_dir": "/tmp/pp-client-test",
            "hub_url": "http://127.0.0.1:50052",
            "sub_token": "tok",
            "core_type": "singbox",
            "core_binary": "/usr/local/bin/sing-box",
            "mixed_port": 17890,
            "mitm_enabled": true,
            "mitm": { "ca_dir": "/tmp/pp-client-test/certs", "hostnames": [], "script_dialect": "Surge" },
            "system_proxy_enabled": false
        }"#;
        let cfg: ClientConfig = serde_json::from_str(json).unwrap();
        assert!(!cfg.tun_enabled);
        assert_eq!(cfg.tun_stack, "mixed");
        assert!(cfg.tun_auto_route);
        assert!(!cfg.clash_api_enabled);
        assert_eq!(cfg.clash_api_port, 9090);
        assert!(cfg.clash_api_secret.is_empty());
        assert_eq!(cfg.clash_api_ui, "zashboard");
    }

    #[test]
    fn load_save_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            "http://127.0.0.1:50052",
            "tok",
            CoreType::SingBox,
            PathBuf::from("/usr/local/bin/sing-box"),
        );
        cfg.hub_url = "http://localhost:50052".to_string();
        cfg.mixed_port = 20000;
        cfg.system_proxy_enabled = true;

        cfg.save().unwrap();
        assert!(dir.path().join("client.json").exists());

        let loaded = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(cfg, loaded);
    }
}
