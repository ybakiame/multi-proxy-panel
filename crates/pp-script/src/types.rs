use serde::{Deserialize, Serialize};

/// 脚本方言（模拟的客户端代理软件 API 风格）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScriptDialect {
    /// Quantumult X 风格（$task / $prefs / $notify）。
    QuantumultX,
    /// Surge 风格（$httpClient / $persistentStore / $notification）。
    Surge,
    /// Loon 风格（同时注入 QX 与 Surge 两套 + $loon 标记）。
    Loon,
}

/// 脚本类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScriptKind {
    /// http-request：arg 为请求描述，$done 返回修改后的请求。
    HttpRequest,
    /// http-response：arg 为响应描述，$done 返回修改后的响应。
    HttpResponse,
    /// cron 定时脚本。
    Cron,
    /// 通用脚本。
    Generic,
}

/// 一次 HTTP 请求的描述。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpRequestSpec {
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
    pub timeout_ms: Option<u64>,
}

/// 一次 HTTP 响应的数据。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpResponseData {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

/// 脚本执行结果（$done 的参数）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScriptOutput(pub serde_json::Value);

impl Default for ScriptOutput {
    fn default() -> Self {
        Self(serde_json::Value::Null)
    }
}

/// 脚本执行资源限制。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScriptLimits {
    /// 整体执行超时（毫秒），默认 5000。
    pub timeout_ms: u64,
    /// QuickJS 运行时内存上限（字节），默认 32MB。
    pub memory_limit_bytes: usize,
}

impl Default for ScriptLimits {
    fn default() -> Self {
        Self {
            timeout_ms: 5000,
            memory_limit_bytes: 32 * 1024 * 1024,
        }
    }
}
