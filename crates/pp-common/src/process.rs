//! Process spawning helpers.

/// Do not create a console window for the spawned child on Windows
/// (`CREATE_NO_WINDOW`, `0x08000000`); no-op on other platforms.
///
/// The desktop client is a GUI-subsystem app: every child process spawned
/// without this flag (core `run`, `version` probes, `reg` for system proxy, …)
/// creates a visible cmd window — at startup this reads as a console window
/// flashing repeatedly. Apply to **every** child process spawned on behalf of
/// the GUI session.
#[cfg(windows)]
pub fn no_window(cmd: &mut std::process::Command) -> &mut std::process::Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW)
}

/// See [`no_window`] (no-op on non-Windows platforms).
#[cfg(not(windows))]
pub fn no_window(cmd: &mut std::process::Command) -> &mut std::process::Command {
    cmd
}

/// Tokio variant of [`no_window`] (tokio's `Command` implements the same
/// `CommandExt` trait on Windows).
#[cfg(windows)]
pub fn no_window_tokio(cmd: &mut tokio::process::Command) -> &mut tokio::process::Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW)
}

/// See [`no_window_tokio`] (no-op on non-Windows platforms).
#[cfg(not(windows))]
pub fn no_window_tokio(cmd: &mut tokio::process::Command) -> &mut tokio::process::Command {
    cmd
}

#[cfg(test)]
mod tests {
    #[test]
    fn no_window_is_applicable_to_std_command() {
        let mut cmd = std::process::Command::new("sing-box");
        // 跨平台仅验证可调用且返回同一命令（Windows 侧 flag 语义由 CI Windows
        // 构建与手工验证覆盖）。
        assert!(super::no_window(&mut cmd).get_program() == "sing-box");
    }

    #[test]
    fn no_window_is_applicable_to_tokio_command() {
        let mut cmd = tokio::process::Command::new("sing-box");
        let _ = super::no_window_tokio(&mut cmd);
    }
}
