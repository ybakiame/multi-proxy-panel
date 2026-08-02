//! 核心引擎桥接抽象（Android libbox 接入预留）。
//!
//! 桌面端核心由 `pp-core` 的 [`pp_core::CoreManager`] 直接 spawn 二进制进程驱动；
//! Android 上无法 spawn 进程，核心将由 Kotlin 侧 libbox（VpnService）驱动。
//! Rust 侧需要一个「插件桥」占位：src-tauri 在 Android 启动时安装桥实现，
//! [`CoreRunner`] 在 Android 下委托给该桥（P1c 实现真正的 Kotlin 通道，
//! 本模块只建抽象与接线）。
//!
//! 桥的生命周期接口（start / stop / is_running）与 `pp-core` 的
//! [`pp_core::CoreManager`] 异步签名兼容；全局注册表用 [`OnceLock`] 保证
//! 只安装一次。

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use pp_common::{CoreType, PanelError, PanelResult};
use pp_core::CoreManager;
use serde_json::Value;

/// 桥接方法返回的盒化 future（手写对象安全异步签名，避免引入额外 crate）。
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// 核心引擎桥：把核心生命周期委托给平台侧实现（Android 下为 Kotlin libbox）。
pub trait CoreEngineBridge: Send + Sync {
    /// 启动核心（`config_json` 为 sing-box/mihomo 配置 JSON）。
    fn start<'a>(&'a self, config_json: &'a Value) -> BoxFuture<'a, PanelResult<()>>;

    /// 停止核心。
    fn stop<'a>(&'a self) -> BoxFuture<'a, PanelResult<()>>;

    /// 核心是否在运行。
    fn is_running<'a>(&'a self) -> BoxFuture<'a, bool>;
}

/// 全局核心引擎桥注册表（只允许安装一次）。
static CORE_ENGINE_BRIDGE: std::sync::OnceLock<Arc<dyn CoreEngineBridge>> =
    std::sync::OnceLock::new();

/// 安装全局核心引擎桥（Android 启动时由 src-tauri 调用；重复安装返回错误）。
pub fn install_core_engine_bridge(bridge: Arc<dyn CoreEngineBridge>) -> PanelResult<()> {
    CORE_ENGINE_BRIDGE
        .set(bridge)
        .map_err(|_| PanelError::Client("核心引擎桥已安装，不能重复安装".into()))
}

/// 读取已安装的核心引擎桥；未安装时返回 `None`。
pub fn core_engine_bridge() -> Option<Arc<dyn CoreEngineBridge>> {
    CORE_ENGINE_BRIDGE.get().cloned()
}

/// [`pp_core::CoreManager`] → [`CoreEngineBridge`] 适配器：
/// 把 `CoreManager` 的调用转发给桥，供 Android 下 [`crate::runner::CoreRunner`]
/// 复用现有 `Box<dyn CoreManager>` 容器。
pub struct CoreEngineBridgeAdapter {
    core_type: CoreType,
    bridge: Arc<dyn CoreEngineBridge>,
}

impl CoreEngineBridgeAdapter {
    /// 构造适配器（记录核心类型以满足 [`CoreManager::core_type`]）。
    pub fn new(core_type: CoreType, bridge: Arc<dyn CoreEngineBridge>) -> Self {
        Self { core_type, bridge }
    }
}

#[async_trait::async_trait]
impl CoreManager for CoreEngineBridgeAdapter {
    fn core_type(&self) -> CoreType {
        self.core_type
    }

    async fn start(&self, config: &Value) -> PanelResult<()> {
        self.bridge.start(config).await
    }

    async fn stop(&self) -> PanelResult<()> {
        self.bridge.stop().await
    }

    async fn restart(&self, config: &Value) -> PanelResult<()> {
        self.stop().await?;
        self.start(config).await
    }

    async fn is_running(&self) -> bool {
        self.bridge.is_running().await
    }

    async fn reload(&self, config: &Value) -> PanelResult<()> {
        self.restart(config).await
    }

    async fn version(&self) -> PanelResult<String> {
        Ok(String::new())
    }

    async fn uptime_secs(&self) -> PanelResult<u64> {
        Ok(0)
    }

    async fn active_inbounds(&self) -> PanelResult<Vec<String>> {
        Ok(vec![])
    }

    async fn last_error(&self) -> PanelResult<String> {
        Ok(String::new())
    }
}
