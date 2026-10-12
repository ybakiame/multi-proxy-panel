//! Windows 平台进程原语实现。
//!
//! 句柄类型说明：windows-sys 0.61 中 `HANDLE` 为 `*mut c_void`，空句柄以
//! `.is_null()` 判断。

use std::io;
use std::path::PathBuf;

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE, QueryFullProcessImageNameW,
    TerminateProcess,
};

/// 持有 Job Object 句柄的 guard；drop 时关闭句柄。
///
/// Job 配置了 `KILL_ON_JOB_CLOSE`：最后一个句柄关闭（含父进程死亡时内核
/// 自动关闭）即终止 Job 内所有进程，因此 guard 必须与管理的子进程同生命周期。
pub struct JobGuard(HANDLE);

impl Drop for JobGuard {
    fn drop(&mut self) {
        // SAFETY: self.0 为 CreateJobObjectW 返回的有效句柄，仅在此关闭一次。
        unsafe {
            CloseHandle(self.0);
        }
    }
}

// SAFETY: Job 句柄是进程内资源，guard 仅持有句柄供 drop 关闭，跨线程移动/共享安全。
unsafe impl Send for JobGuard {}
unsafe impl Sync for JobGuard {}

/// spawn 前配置：Windows 无 pre-spawn 动作（Job 挂接在 post-spawn 完成）。
pub fn configure_pre_spawn(_cmd: &mut std::process::Command) {}

/// post-spawn 绑定：将子进程挂入 KILL_ON_JOB_CLOSE 的 Job Object。
/// 挂接失败（如外层 Job 限制嵌套）降级为未绑定，由 PID 收割兜底。
pub fn bind_post_spawn(child: &std::process::Child) -> super::LifecycleGuard {
    match bind_to_job(child) {
        Ok(job) => super::LifecycleGuard::new(true, Some(job)),
        Err(_) => super::LifecycleGuard::new(false, None),
    }
}

fn bind_to_job(child: &std::process::Child) -> io::Result<JobGuard> {
    use std::os::windows::io::AsRawHandle;
    unsafe {
        // SAFETY: 全空默认属性的匿名 Job；错误路径均关闭句柄。
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return Err(io::Error::last_os_error());
        }
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let ok = SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &raw const info as *const core::ffi::c_void,
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        if ok == 0 {
            let e = io::Error::last_os_error();
            CloseHandle(job);
            return Err(e);
        }
        if AssignProcessToJobObject(job, child.as_raw_handle() as HANDLE) == 0 {
            let e = io::Error::last_os_error();
            CloseHandle(job);
            return Err(e);
        }
        Ok(JobGuard(job))
    }
}

/// `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` 探测进程是否存在。
pub fn is_process_alive(pid: u32) -> bool {
    unsafe {
        // SAFETY: 仅查询打开随即关闭。
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return false;
        }
        CloseHandle(h);
        true
    }
}

/// `QueryFullProcessImageNameW` 取进程可执行文件完整路径。
pub fn process_exe_path(pid: u32) -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    unsafe {
        // SAFETY: 查询打开；缓冲区定长，len 为 in/out 参数。
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return None;
        }
        let mut buf = vec![0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len);
        CloseHandle(h);
        if ok == 0 {
            return None;
        }
        buf.truncate(len as usize);
        Some(PathBuf::from(std::ffi::OsString::from_wide(&buf)))
    }
}

/// `TerminateProcess` 强杀。调用方须先经 [`super::process_exe_path`] 校验身份。
pub fn kill_process(pid: u32) -> io::Result<()> {
    unsafe {
        // SAFETY: 终止后关闭句柄；失败透传 OS 错误。
        let h = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if h.is_null() {
            return Err(io::Error::last_os_error());
        }
        let ok = TerminateProcess(h, 1);
        CloseHandle(h);
        if ok == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}
