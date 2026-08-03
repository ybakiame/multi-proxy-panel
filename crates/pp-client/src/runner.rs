//! 核心进程运行器。
//!
//! 封装 `pp-core` 的 [`CoreManagerFactory`]，为客户端提供统一的
//! 核心进程启动 / 停止 / 状态查询入口。

use std::path::Path;

use pp_common::{CoreType, PanelResult};
use serde_json::Value;

// Android 下核心由引擎桥驱动，不 spawn 二进制，故工厂仅在非 android 平台使用。
#[cfg(not(target_os = "android"))]
use pp_core::CoreManagerFactory;

/// 核心进程运行器。
pub struct CoreRunner {
    inner: Box<dyn pp_core::CoreManager>,
}

impl CoreRunner {
    /// 基于核心类型 / 二进制路径 / 配置目录创建运行器。
    ///
    /// Android 下无法 spawn 核心二进制（核心由 Kotlin 侧 libbox 驱动），
    /// 忽略 `binary_path` / `config_dir`，改为委托给已安装的核心引擎桥
    /// （见 [`crate::core_engine`]；P1c 前为占位桥，未安装时报明确错误）。
    pub fn create(core_type: CoreType, binary_path: &Path, config_dir: &Path) -> PanelResult<Self> {
        #[cfg(target_os = "android")]
        {
            let _ = (binary_path, config_dir);
            return Self::create_from_bridge(core_type);
        }
        #[cfg(not(target_os = "android"))]
        {
            let inner = CoreManagerFactory::create(core_type, binary_path, config_dir)?;
            Ok(Self { inner })
        }
    }

    /// Android 下从已安装的核心引擎桥构造运行器（忽略二进制路径）。
    ///
    /// 未安装桥时返回明确错误「Android 核心引擎桥未初始化」，避免 P0 启动代理时
    /// 出现 spawn 失败乱码。仅在 Android 或测试构建下编译（桌面生产构建无此路径）。
    #[cfg(any(target_os = "android", test))]
    fn create_from_bridge(core_type: CoreType) -> PanelResult<Self> {
        use crate::core_engine::{CoreEngineBridgeAdapter, core_engine_bridge};
        let bridge = core_engine_bridge()
            .ok_or_else(|| pp_common::PanelError::Client("Android 核心引擎桥未初始化".into()))?;
        let inner: Box<dyn pp_core::CoreManager> =
            Box::new(CoreEngineBridgeAdapter::new(core_type, bridge));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_engine::{BoxFuture, CoreEngineBridge, install_core_engine_bridge};
    use pp_common::PanelError;
    use std::sync::{Arc, Mutex};

    /// 记录运行态的假桥，用于验证 Android 桥路径的转发。
    #[derive(Default)]
    struct MockBridge {
        running: Mutex<bool>,
        /// 最近一次 `start` 收到的核心类型（供转发断言）。
        received_core_type: Mutex<Option<CoreType>>,
    }

    impl CoreEngineBridge for MockBridge {
        fn start<'a>(
            &'a self,
            core_type: CoreType,
            config_json: &'a Value,
        ) -> BoxFuture<'a, PanelResult<()>> {
            Box::pin(async move {
                *self.received_core_type.lock().unwrap() = Some(core_type);
                assert_eq!(config_json, &serde_json::json!({"tag": "mock-config"}));
                *self.running.lock().unwrap() = true;
                Ok(())
            })
        }

        fn stop<'a>(&'a self) -> BoxFuture<'a, PanelResult<()>> {
            Box::pin(async move {
                *self.running.lock().unwrap() = false;
                Ok(())
            })
        }

        fn is_running<'a>(&'a self) -> BoxFuture<'a, bool> {
            Box::pin(async move { *self.running.lock().unwrap() })
        }
    }

    /// 全局注册表是 `OnceLock`，进程内只能安装一次；Rust 测试并行跑在同一进程，
    /// 故把「未安装报错」与「安装后转发」合并为单个测试保证串行顺序。
    #[tokio::test]
    async fn bridge_registry_install_and_lifecycle_forwarding() {
        // 未安装：create_from_bridge 报明确错误。
        let err = match CoreRunner::create_from_bridge(CoreType::SingBox) {
            Ok(_) => panic!("未安装桥时 create_from_bridge 应报错"),
            Err(e) => e,
        };
        assert!(
            err.to_string().contains("Android 核心引擎桥未初始化"),
            "意外错误信息：{err}"
        );

        // 安装 MockBridge 后：create 成功，start/stop/is_running 转发到桥。
        let mock = Arc::new(MockBridge::default());
        install_core_engine_bridge(mock.clone()).unwrap();

        let runner = CoreRunner::create_from_bridge(CoreType::SingBox).unwrap();
        let config = serde_json::json!({"tag": "mock-config"});
        assert!(!runner.is_running().await);
        runner.start(&config).await.unwrap();
        assert!(runner.is_running().await);
        assert!(mock.is_running().await);
        // 桥收到的 core_type 应为构造运行器时传入的 SingBox（转发断言）。
        assert_eq!(
            *mock.received_core_type.lock().unwrap(),
            Some(CoreType::SingBox),
            "桥 start 应收到构造运行器时的核心类型"
        );
        runner.stop().await.unwrap();
        assert!(!runner.is_running().await);
        assert!(!mock.is_running().await);
    }

    /// PanelError::Client 的错误信息应透传中文提示（桌面路径不受影响）。
    #[test]
    fn bridge_error_message_is_chinese() {
        let err = PanelError::Client("Android 核心引擎桥未初始化".into());
        assert!(err.to_string().contains("Android 核心引擎桥未初始化"));
    }
}
