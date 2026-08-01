//! pp-client — 桌面客户端核心库。
//!
//! 提供客户端配置（[`config`]）、分享链接解析（[`share_link`]）、双核心节点转换
//! （[`node_convert`]）、通用订阅管理（[`subscription`]）、三方配置片段导入
//! （[`import`]）、Profile 模板与复写（[`profile`]）、核心配置合成（[`core_config`]）、
//! 系统代理（[`sysproxy`]）、核心运行器（[`runner`]）、MITM 构建（[`mitm`]）与运行状态编排（[`state`]）。

#![allow(clippy::result_large_err)]

pub mod config;
pub mod core_config;
pub mod cores;
pub mod http_exec;
pub mod import;
pub mod mitm;
pub mod node_convert;
pub mod privilege;
pub mod profile;
pub mod remote;
pub mod runner;
pub mod share_link;
pub mod state;
pub mod subscription;
pub mod sysproxy;

pub use config::*;
pub use core_config::*;
pub use cores::*;
pub use http_exec::*;
pub use import::*;
pub use mitm::*;
pub use node_convert::*;
pub use privilege::*;
pub use profile::*;
pub use remote::*;
pub use runner::*;
pub use share_link::*;
pub use state::*;
pub use subscription::*;
pub use sysproxy::*;

/// 归一化 GitHub 资源 URL：`github.com/<owner>/<repo>/{blob,raw}/<branch>/<path>` →
/// `raw.githubusercontent.com/<owner>/<repo>/<branch>/<path>`；其他 URL 原样返回。
///
/// 供订阅拉取 / 远程资源拉取 / 导入脚本 URL / 远端嗅探在进入 HTTP 请求前统一调用，
/// 避免 GitHub 网页端 blob 链接被当作原始文件拉取。
pub fn normalize_resource_url(url: &str) -> String {
    let Some(rest) = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
    else {
        return url.to_string();
    };
    let mut parts = rest.splitn(5, '/');
    let (Some(owner), Some(repo), Some(kind), Some(branch)) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return url.to_string();
    };
    if owner.is_empty() || repo.is_empty() || branch.is_empty() {
        return url.to_string();
    }
    match kind {
        "blob" | "raw" => {
            let path = parts.next().unwrap_or("");
            let suffix = if path.is_empty() {
                String::new()
            } else {
                format!("/{path}")
            };
            format!("https://raw.githubusercontent.com/{owner}/{repo}/{branch}{suffix}")
        }
        _ => url.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_resource_url;

    #[test]
    fn normalizes_github_blob_and_raw_to_raw_githubusercontent() {
        assert_eq!(
            normalize_resource_url(
                "https://github.com/SagerNet/sing-box/blob/main/transport/splithttp/client.go"
            ),
            "https://raw.githubusercontent.com/SagerNet/sing-box/main/transport/splithttp/client.go"
        );
        assert_eq!(
            normalize_resource_url("https://github.com/owner/repo/raw/main/sub/module.sgmodule"),
            "https://raw.githubusercontent.com/owner/repo/main/sub/module.sgmodule"
        );
    }

    #[test]
    fn leaves_already_raw_and_non_github_urls_unchanged() {
        assert_eq!(
            normalize_resource_url("https://raw.githubusercontent.com/o/r/main/x.js"),
            "https://raw.githubusercontent.com/o/r/main/x.js"
        );
        assert_eq!(
            normalize_resource_url("https://example.com/not/github/blob/main/x.js"),
            "https://example.com/not/github/blob/main/x.js"
        );
        assert_eq!(
            normalize_resource_url("http://github.com/o/r/tree/main/x.js"),
            "http://github.com/o/r/tree/main/x.js"
        );
        // 结构不完整（缺 branch / path）时原样返回。
        assert_eq!(
            normalize_resource_url("https://github.com/owner/repo"),
            "https://github.com/owner/repo"
        );
    }
}
