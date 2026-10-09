//! 核心进程生命周期辅助（ADR-0012 D1/D2）。
//!
//! - [`spawn_core`]：经 `pp_common::spawn_guarded` spawn 核心子进程并完成
//!   std → tokio 转换（OS 级父子绑定：父死子亡）；
//! - PID 文件（`<config_dir>/<core>.pid`）：`start` 成功写入、`stop` 删除、
//!   `start` 前 [`reap_stale`] 收割上次实例遗留的孤儿进程。

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use pp_common::LifecycleGuard;
use tokio::process::{ChildStderr, ChildStdout};

/// PID 文件内容：pid + exe 路径（身份校验用）+ 启动时间（观测用）。
#[derive(serde::Serialize, serde::Deserialize)]
struct PidFile {
    pid: u32,
    exe: PathBuf,
    started_at_unix: u64,
}

/// 核心 spawn 结果：std 子进程 + 父子绑定 guard + tokio 化的 stdio。
///
/// 子进程保持 std 形态：tokio 不提供 `Child::from_std`（无法接管 std 子进程
/// 的 reap 集成）；std Child 的 kill/try_wait/id 均为同步 API，管理器直接
/// 使用即可，stdio 则经 `ChildStdout::from_std` / `ChildStderr::from_std`
/// 转为 tokio 句柄供异步日志读取。
pub struct CoreSpawn {
    pub child: std::process::Child,
    pub guard: LifecycleGuard,
    pub stdout: Option<ChildStdout>,
    pub stderr: Option<ChildStderr>,
}

/// Spawn 核心子进程并绑定其生命周期到本进程（ADR-0012 D1），完成
/// std → tokio 的 Child / stdio 转换。`core` 为错误信息中的核心名。
pub fn spawn_core<I, S>(core: &str, binary: &Path, args: I) -> Result<CoreSpawn, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = std::process::Command::new(binary);
    cmd.args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    pp_common::no_window(&mut cmd);
    let (mut std_child, guard) =
        pp_common::spawn_guarded(&mut cmd).map_err(|e| format!("failed to spawn {core}: {e}"))?;
    if !guard.is_bound() {
        tracing::warn!("核心进程父子绑定未生效（平台不支持或挂接失败），依赖 PID 收割兜底");
    }
    // 先从 std Child 取出 stdio，再分别 tokio 化。
    let stdout = std_child.stdout.take();
    let stderr = std_child.stderr.take();
    let stdout = stdout
        .map(ChildStdout::from_std)
        .transpose()
        .map_err(|e| format!("failed to convert stdout: {e}"))?;
    let stderr = stderr
        .map(ChildStderr::from_std)
        .transpose()
        .map_err(|e| format!("failed to convert stderr: {e}"))?;
    Ok(CoreSpawn {
        child: std_child,
        guard,
        stdout,
        stderr,
    })
}

/// 进程身份校验（PID 复用防护）：进程 exe 与管理的二进制路径一致。
fn exe_matches(pid: u32, binary: &Path) -> bool {
    let Some(exe) = pp_common::process_exe_path(pid) else {
        return false;
    };
    let exe = exe.canonicalize().unwrap_or(exe);
    let bin = binary
        .canonicalize()
        .unwrap_or_else(|_| binary.to_path_buf());
    #[cfg(windows)]
    {
        exe.to_string_lossy()
            .eq_ignore_ascii_case(&bin.to_string_lossy())
    }
    #[cfg(not(windows))]
    {
        exe == bin
    }
}

/// `start` 前收割上次实例遗留的核心进程（ADR-0012 D2）。
///
/// pidfile 存在且进程存活且 exe 匹配时强杀；进程已死、exe 不匹配（PID 复用）
/// 或 pidfile 损坏时仅清理 pidfile。全部失败降级为告警，不阻塞启动。
pub fn reap_stale(pid_path: &Path, binary: &Path) {
    let Ok(content) = std::fs::read_to_string(pid_path) else {
        return;
    };
    let Ok(record) = serde_json::from_str::<PidFile>(&content) else {
        tracing::warn!(path = %pid_path.display(), "pidfile 损坏，直接清理");
        let _ = std::fs::remove_file(pid_path);
        return;
    };
    if pp_common::is_process_alive(record.pid) {
        if exe_matches(record.pid, binary) {
            tracing::warn!(pid = record.pid, "收割遗留核心进程（上次实例未正常退出）");
            if let Err(e) = pp_common::kill_process(record.pid) {
                tracing::warn!(pid = record.pid, error = %e, "遗留核心进程强杀失败");
            }
        } else {
            tracing::info!(
                pid = record.pid,
                "pidfile 指向的 PID 已被其他进程复用，跳过收割"
            );
        }
    }
    let _ = std::fs::remove_file(pid_path);
}

/// `start` 成功后写入 PID 文件（失败仅告警，不影响运行）。
pub fn write_pid_file(pid_path: &Path, pid: u32, binary: &Path) {
    let record = PidFile {
        pid,
        exe: binary.to_path_buf(),
        started_at_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    };
    match serde_json::to_string(&record) {
        Ok(json) => {
            if let Err(e) = std::fs::write(pid_path, json) {
                tracing::warn!(error = %e, "PID 文件写入失败");
            }
        }
        Err(e) => tracing::warn!(error = %e, "PID 文件序列化失败"),
    }
}

/// `stop` / 进程退出后删除 PID 文件（best-effort）。
pub fn remove_pid_file(pid_path: &Path) {
    let _ = std::fs::remove_file(pid_path);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sleep_pid() -> (std::process::Child, u32) {
        let child = std::process::Command::new("sleep")
            .arg("30")
            .spawn()
            .expect("spawn sleep");
        let pid = child.id();
        (child, pid)
    }

    /// sleep 可执行文件的真实路径（canonicalize 后用于 exe 匹配断言）。
    fn sleep_exe() -> PathBuf {
        which_sleep()
            .canonicalize()
            .unwrap_or_else(|_| which_sleep())
    }

    fn which_sleep() -> PathBuf {
        // /proc/<pid>/exe 会解析到真实路径（如 /usr/bin/sleep 或 coreutils 实际路径），
        // 直接查 PATH 可能与 /proc 解析不一致，故以 /proc/self 解析 PATH 中的 sleep。
        for dir in ["/usr/bin", "/bin", "/usr/sbin", "/sbin"] {
            let p = Path::new(dir).join("sleep");
            if p.exists() {
                return p;
            }
        }
        panic!("sleep binary not found");
    }

    #[test]
    fn reap_stale_no_pidfile_is_noop() {
        let dir = std::env::temp_dir().join(format!("pp-test-{}", std::process::id()));
        let pid_path = dir.join("sing-box.pid");
        reap_stale(&pid_path, Path::new("/nonexistent"));
        assert!(!pid_path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn write_and_reap_kills_matching_orphan() {
        let dir = std::env::temp_dir().join(format!("pp-test-reap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pid_path = dir.join("sing-box.pid");
        let binary = sleep_exe();

        let (mut child, pid) = sleep_pid();
        write_pid_file(&pid_path, pid, &binary);
        assert!(pid_path.exists());
        assert!(pp_common::is_process_alive(pid));

        reap_stale(&pid_path, &binary);

        assert!(!pid_path.exists(), "收割后 pidfile 应删除");
        let status = child.wait().unwrap();
        assert!(!status.success(), "遗留进程应被强杀");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn reap_stale_skips_pid_reuse() {
        // PID 复用防护：pidfile 记录的是 sleep 的 pid，但管理的二进制是另一个
        // 路径 → exe 不匹配 → 不得误杀，仅清理 pidfile。
        let dir = std::env::temp_dir().join(format!("pp-test-reuse-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pid_path = dir.join("sing-box.pid");
        let wrong_binary = PathBuf::from("/bin/false");

        let (mut child, pid) = sleep_pid();
        write_pid_file(&pid_path, pid, &wrong_binary);
        reap_stale(&pid_path, &wrong_binary);

        assert!(!pid_path.exists(), "pidfile 应清理");
        assert!(pp_common::is_process_alive(pid), "exe 不匹配时不得误杀");
        child.kill().unwrap();
        let _ = child.wait();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn reap_stale_removes_dead_pidfile() {
        let dir = std::env::temp_dir().join(format!("pp-test-dead-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let pid_path = dir.join("sing-box.pid");
        let binary = sleep_exe();

        let (mut child, pid) = sleep_pid();
        write_pid_file(&pid_path, pid, &binary);
        child.kill().unwrap();
        let _ = child.wait();

        reap_stale(&pid_path, &binary);
        assert!(!pid_path.exists(), "进程已死时 pidfile 应清理");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
