//! 端口占用前置诊断（ADR-0012 D4）。
//!
//! 核心 spawn 前探测将绑定的本地端口；被占用时返回指明占用者的中文错误，
//! 替代核心启动失败后直抛的原始 FATAL（如 `listen tcp 127.0.0.1:9090:
//! bind: ...`）。
//!
//! 调用方须先执行 PID 收割（[`crate::runner::CoreRunner::reap_stale`]）：
//! 上次实例遗留的孤儿进程占用端口由收割自愈，此处剩余的占用均为外部冲突。

use pp_common::{PanelError, PanelResult};

/// 端口探测函数类型（[`ClientState`](crate::state::ClientState) 经
/// `set_port_probe` 注入，生产默认为 [`ensure_ports_available`]）。
pub type PortProbe = std::sync::Arc<dyn Fn(&[(u16, &str)]) -> PanelResult<()> + Send + Sync>;

/// 探测端口占用：全部被探测端口可绑定（127.0.0.1）时返回 `Ok`；
/// 任一被占用即返回错误，消息指明占用者进程名与 PID（反查失败时退化为
/// 通用提示）。
///
/// `ports` 为 `(端口, 用途标签)` 列表。探测为 bind 后立即释放，与核心随后
/// 的真实绑定存在 TOCTOU 窗口；该窗口极小，兜底仍是核心自身的报错。
pub fn ensure_ports_available(ports: &[(u16, &str)]) -> PanelResult<()> {
    for &(port, label) in ports {
        let probe = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port));
        if probe.is_ok() {
            continue;
        }
        let msg = match pp_common::port_owner(port) {
            Some(owner) => format!(
                "端口 {port}（{label}）被进程 {}（PID {}）占用，请关闭该进程或在设置中修改端口",
                owner.name, owner.pid
            ),
            None => {
                format!("端口 {port}（{label}）已被其他程序占用，请释放该端口或在设置中修改端口")
            }
        };
        return Err(PanelError::Client(msg));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn free_ports_pass() {
        // 绑定一个临时端口拿空闲端口号后释放，探测应通过。
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        ensure_ports_available(&[(port, "测试")]).expect("空闲端口应通过");
    }

    #[test]
    fn occupied_port_errors_with_port_and_label() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let err = ensure_ports_available(&[(port, "Clash API")]).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains(&port.to_string()), "错误应含端口: {msg}");
        assert!(msg.contains("Clash API"), "错误应含用途标签: {msg}");
        // Linux 上应能反查到占用者（本测试进程）；其他平台反查可能失败，仅
        // 校验通用提示存在。
        #[cfg(target_os = "linux")]
        assert!(msg.contains("PID"), "Linux 应指明占用者: {msg}");
    }
}
