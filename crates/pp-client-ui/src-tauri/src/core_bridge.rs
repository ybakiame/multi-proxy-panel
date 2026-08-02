//! Android 核心引擎桥（占位实现，仅 Android 编译）。
//!
//! Android 上核心由 Kotlin 侧 libbox（VpnService）驱动，Rust 侧无法 spawn
//! 二进制。P1a 阶段只建抽象与接线：本模块在 Android 启动时向 pp-client 安装
//! 占位桥，对 `start` / `stop` 返回「libbox 尚未接入（P1c）」的明确中文错误，
//! `is_running` 恒为 `false`——保证 P0 启动代理时报错清晰，而非 spawn 失败乱码。
//!
//! P1c 实现真实 Kotlin ↔ Rust 通道后，以真实桥实现替换本占位桥即可。

use std::sync::Arc;

use pp_client::core_engine::{install_core_engine_bridge, BoxFuture, CoreEngineBridge};
use pp_common::{PanelError, PanelResult};
use serde_json::Value;

/// 占位桥：P1c 接入前 `start` / `stop` 一律报「未接入」错误，`is_running` 恒为 `false`。
pub struct AndroidCoreBridge;

impl CoreEngineBridge for AndroidCoreBridge {
    fn start<'a>(&'a self, _config_json: &'a Value) -> BoxFuture<'a, PanelResult<()>> {
        Box::pin(async {
            Err(PanelError::Client(
                "libbox 尚未接入（P1c），当前版本暂不支持在 Android 启动代理".into(),
            ))
        })
    }

    fn stop<'a>(&'a self) -> BoxFuture<'a, PanelResult<()>> {
        Box::pin(async {
            Err(PanelError::Client(
                "libbox 尚未接入（P1c），当前版本暂不支持在 Android 停止代理".into(),
            ))
        })
    }

    fn is_running<'a>(&'a self) -> BoxFuture<'a, bool> {
        Box::pin(async { false })
    }
}

/// 安装占位桥到 pp-client 全局注册表（Android 启动时调用一次）。
pub fn install_android_core_bridge() {
    match install_core_engine_bridge(Arc::new(AndroidCoreBridge)) {
        Ok(()) => tracing::info!("已安装 Android 核心引擎桥（占位实现，P1c 接入 libbox）"),
        Err(e) => tracing::warn!("核心引擎桥安装失败：{e}"),
    }
}
