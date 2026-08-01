//! 核心进程运行器。
//!
//! 封装 `pp-core` 的 [`CoreManagerFactory`]，为客户端提供统一的
//! 核心进程启动 / 停止 / 状态查询入口。

use std::path::Path;

use pp_common::{CoreType, PanelResult};
use pp_core::CoreManagerFactory;
use serde_json::Value;

/// 核心进程运行器。
pub struct CoreRunner {
    inner: Box<dyn pp_core::CoreManager>,
}

impl CoreRunner {
    /// 基于核心类型 / 二进制路径 / 配置目录创建运行器。
    pub fn create(core_type: CoreType, binary_path: &Path, config_dir: &Path) -> PanelResult<Self> {
        let inner = CoreManagerFactory::create(core_type, binary_path, config_dir)?;
        Ok(Self { inner })
    }

    /// 启动核心（`config_json` 为 sing-box/mihomo 配置）。
    pub async fn start(&self, config_json: &Value) -> PanelResult<()> {
        self.inner.start(config_json).await
    }

    /// 停止核心。
    pub async fn stop(&self) -> PanelResult<()> {
        self.inner.stop().await
    }

    /// 核心是否在运行。
    pub async fn is_running(&self) -> bool {
        self.inner.is_running().await
    }
}
