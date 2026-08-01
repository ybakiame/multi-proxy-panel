use pp_common::{PanelError, PanelResult};

/// `PanelError::Script` 的辅助构造函数。
pub fn script_err(msg: impl Into<String>) -> PanelError {
    PanelError::Script(msg.into())
}

/// `PanelResult` 的脚本错误快捷别名。
pub type ScriptResult<T> = PanelResult<T>;
