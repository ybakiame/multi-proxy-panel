//! Unix 平台进程原语实现（Linux / macOS / Android）。

use std::io;
use std::path::PathBuf;

/// Unix 侧无 post-spawn 资源需要持有（pdeathsig 在子进程侧生效），占位类型。
pub struct JobGuard;

/// spawn 前配置：Linux / Android（含 WSL）设置 `PR_SET_PDEATHSIG`，父进程
/// 死亡时内核向子进程发 SIGKILL；macOS 无等价机制，跳过（ADR-0012 D1）。
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

/// 进程可执行文件路径：Linux / Android 读 `/proc/<pid>/exe`，macOS 走
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

/// 反查监听端口的占用进程：`/proc/net/tcp{,6}` 找 LISTEN socket inode →
/// 扫 `/proc/*/fd` 反查 PID → `/proc/<pid>/comm` 取名。macOS 无 /proc，
/// 各步读取静默失败，最终返回 `None`。
pub fn port_owner(port: u16) -> Option<super::PortOwner> {
    for inode in listen_socket_inodes(port) {
        if let Some(pid) = find_pid_by_socket_inode(&inode) {
            let name = process_name(pid).unwrap_or_else(|| "<unknown>".into());
            return Some(super::PortOwner { pid, name });
        }
    }
    None
}

/// 解析 /proc/net/tcp{,6}，返回监听指定端口的 socket inode 列表。
fn listen_socket_inodes(port: u16) -> Vec<String> {
    let mut out = Vec::new();
    for table in ["/proc/net/tcp", "/proc/net/tcp6"] {
        let Ok(content) = std::fs::read_to_string(table) else {
            continue;
        };
        for line in content.lines().skip(1) {
            let cols: Vec<&str> = line.split_whitespace().collect();
            // local_address 列形如 "0100007F:2382"（hex IP:hex 端口）；st=0A 为 LISTEN；
            // inode 在第 10 列（索引 9）。
            if cols.len() > 9
                && cols[3] == "0A"
                && let Some((_, port_hex)) = cols[1].split_once(':')
                && u16::from_str_radix(port_hex, 16).ok() == Some(port)
            {
                out.push(cols[9].to_string());
            }
        }
    }
    out
}

/// 扫 /proc/*/fd 反查持有指定 socket inode 的 PID（无权限的进程目录静默跳过）。
fn find_pid_by_socket_inode(inode: &str) -> Option<u32> {
    let needle = format!("socket:[{inode}]");
    for entry in std::fs::read_dir("/proc").ok()?.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Ok(pid) = name.parse::<u32>() else {
            continue;
        };
        let Ok(fds) = std::fs::read_dir(entry.path().join("fd")) else {
            continue;
        };
        for fd in fds.flatten() {
            if let Ok(target) = std::fs::read_link(fd.path())
                && target.to_string_lossy() == needle
            {
                return Some(pid);
            }
        }
    }
    None
}

/// /proc/<pid>/comm 取进程名。
fn process_name(pid: u32) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .ok()
        .map(|s| s.trim().to_string())
}
