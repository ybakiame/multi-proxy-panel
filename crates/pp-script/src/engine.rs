use crate::types::{ScriptKind, ScriptOutput};
use pp_common::PanelResult;

/// 脚本引擎抽象：引擎无关，为未来第二个后端（Apple JavaScriptCore，feature-gated `engine-jsc`）
/// 预留接口。
///
/// 每个引擎实例绑定一个宿主能力集合与方言。构造签名约定（各后端保持一致）：
///
/// ```ignore
/// QuickJsEngine::new(
///     host: Arc<ScriptHost>,
///     dialect: ScriptDialect,
///     limits: ScriptLimits,
///     script_name: String,
/// ) -> PanelResult<Self>
/// ```
pub trait ScriptEngine {
    /// 执行一段脚本源码。
    ///
    /// - `kind`：脚本类型，`HttpRequest`/`HttpResponse` 时会把 `arg` 注入为全局 `$request`/`$response`。
    /// - `arg`：脚本参数（如 http-request 的请求描述 / http-response 的响应描述）。
    /// - `argument`：Surge/Loon 模块的 `argument=` 模板替换后的字符串；为 `Some` 时注入全局
    ///   `$argument`（JS 字符串），供脚本读取模块参数。
    ///
    /// 返回 `$done(...)` 的参数；若脚本超时未调用 `$done`，返回空输出并记录警告。
    #[allow(async_fn_in_trait)]
    async fn run_script(
        &mut self,
        source: &str,
        kind: ScriptKind,
        arg: Option<serde_json::Value>,
        argument: Option<&str>,
    ) -> PanelResult<ScriptOutput>;
}
