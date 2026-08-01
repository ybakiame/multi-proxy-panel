//! pp-script — 客户端代理软件的 JS 脚本引擎层（引擎无关抽象 + rquickjs 后端 + QX/Surge/Loon 方言 API 注入层）。

pub mod api;
pub mod dialect;
pub mod engine;
pub mod engine_quickjs;
pub mod error;
pub mod host;
pub mod types;

pub use dialect::*;
pub use engine::*;
pub use engine_quickjs::*;
pub use error::*;
pub use host::*;
pub use types::*;
