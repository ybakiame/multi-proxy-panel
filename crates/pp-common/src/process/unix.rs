//! Unix 平台进程原语实现（Linux / macOS）。

use std::io;
use std::path::PathBuf;

/// Unix 侧无 post-spawn 资源需要持有（pdeathsig 在子进程侧生效），占位类型。
pub struct JobGuard;

/// spawn 前配置：Linux（含 WSL）设置 `PR_SET_PDEATHSIG`，父进程
/// 死亡时内核向子进程发 SIGKILL；macOS 无等价机制，跳过。
pub fn configure_pre_spawn(cmd: &mut std::process::Command) {
    #[cfg(not(target_os = "macos"))]
    {
        use std::os::unix::process::CommandExt;
        let parent_pid = std::process::id();
        unsafe {
            cmd.pre_exec(move || {
                // SAFETY: pre_exec 回调内只做 async-signal-safe 的 syscall。
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                // fork/prctl 竞态兜底：父进程可能已先于 prctl 死亡（此时 ppid 变化）。
                if libc::getppid() as u32 != parent_pid {
                    libc::raise(libc::SIGKILL);
                }
                Ok(())
            });
        }
    }
    let _ = cmd;
}

/// post-spawn 绑定：Unix 无可持有资源，仅标记绑定是否生效（macOS 降级）。
pub fn bind_post_spawn(_child: &std::process::Child) -> super::LifecycleGuard {
    let _ = JobGuard;
    super::LifecycleGuard::new(cfg!(not(target_os = "macos")))
}

/// `kill(pid, 0)` 探测：返回 0 或 EPERM 均表示进程存在。
pub fn is_process_alive(pid: u32) -> bool {
    if unsafe { libc::kill(pid as libc::pid_t, 0) } == 0 {
        return true;
    }
    matches!(io::Error::last_os_error().raw_os_error(), Some(e) if e == libc::EPERM)
}

/// 进程可执行文件路径：Linux 读 `/proc/<pid>/exe`，macOS 走
/// `proc_pidpath`。
pub fn process_exe_path(pid: u32) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let mut buf = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
        let ret =
            unsafe { libc::proc_pidpath(pid as i32, buf.as_mut_ptr().cast(), buf.len() as u32) };
        if ret <= 0 {
            return None;
        }
        buf.truncate(ret as usize);
        let s = String::from_utf8_lossy(&buf).into_owned();
        return Some(PathBuf::from(s));
    }
    #[cfg(not(target_os = "macos"))]
    std::fs::read_link(format!("/proc/{pid}/exe")).ok()
}

/// SIGKILL 强杀。调用方须先经 [`super::process_exe_path`] 校验身份。
pub fn kill_process(pid: u32) -> io::Result<()> {
    if unsafe { libc::kill(pid as libc::pid_t, libc::SIGKILL) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
