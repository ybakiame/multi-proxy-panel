# 客户端核心进程生命周期管理设计

- **Date:** 2026-10-09
- **Status:** 已评审（用户确认方案 A + 端口报错指明占用者）
- **Related:** ADR-0012（决策记录）、ADR-0007（单壳双目标）、ADR-0010（内嵌 pinned 核心）

## 背景与问题

桌面端核心（sing-box）由 `pp-core::SingBoxProcessManager` spawn 为子进程，进程句柄
仅存于内存（`RwLock<Option<Child>>`）。实证故障：应用在 WSL 中冻结后被强制关闭，
无任何清理机制，孤儿 sing-box 继续持有 `127.0.0.1:9090`（Clash API）；WSL 的
localhost 转发使该占用对 Windows 侧可见，Windows 上新启动的实例 bind 失败，用户
只看到 sing-box 原始 FATAL 日志。

现状四个层次全部缺失：

1. 无 OS 级父子绑定：父进程被 SIGKILL/结束任务时核心必然成孤儿；
2. 无 PID 文件 / 启动收割：重启应用后对上次遗留的核心无感知；
3. 无退出清理：`.run()` 无 `RunEvent::ExitRequested` 处理，关窗不停止核心；
4. 端口冲突无诊断：原始 FATAL 直接抛给用户。

Android 不在范围内：核心由 Kotlin VPN 服务驱动，生命周期由 OS 托管。

## 分层设计（纵深防御）

### §1 OS 级父子绑定（pp-common::process）

新增 `spawn_guarded()`：spawn 时把子进程生命周期绑定到父进程——父进程以任何方式
死亡（崩溃、结束任务、SIGKILL），核心立即被 OS 回收。

- **Windows**：post-spawn 将子进程挂入 Job Object
  （`JOBOBJECT_EXTENDED_LIMIT_INFORMATION` + `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`），
  Job 句柄封装为 `LifecycleGuard`，由进程管理器与 `Child` 同生命周期持有；
  挂接失败（如嵌套 Job 限制）降级为空 guard 并告警，不阻塞启动。
- **Linux / WSL**：std Command `pre_exec` 设置 `PR_SET_PDEATHSIG = SIGKILL`，
  并在 `pre_exec` 内复核 `getppid()` 兜底 fork/prctl 竞态；spawn 后
  `tokio::process::Child::from_std` 转回 tokio（tokio Command 不支持 pre_exec）。
- **macOS**：无等价轻量机制，跳过（§2 兜底），文档注明。

### §2 PID 文件 + 启动收割（pp-core manager）

- `start()` 成功后写 `<config_dir>/sing-box.pid`（JSON：pid / exe / started_at）；
  `stop()` 删除。
- `start()` 前收割：pidfile 存在且进程存活 → 校验进程 exe 路径与当前管理的二进制
  一致（防 PID 复用误杀第三方进程）→ 一致则强杀回收；不一致仅删除 pidfile。
- 覆盖：旧版本遗留孤儿、macOS、§1 未生效的边角。
- pp-common::process 配套原语：`is_process_alive(pid)`、`process_exe_path(pid)`、
  `kill_process(pid)`（Linux 走 `/proc` + `kill(2)`；Windows 走 `OpenProcess` /
  `QueryFullProcessImageNameW` / `TerminateProcess`）。

### §3 退出清理（apps/client/src-tauri，桌面目标）

`.run()` 改为 `.build()` + `app.run()`：`RunEvent::ExitRequested` 时 best-effort
执行核心停止 + 系统代理恢复（`block_on`，失败仅告警不阻塞退出）。只覆盖正常退出；
冻结被杀场景收不到事件，由 §1/§2 兜底，本层不为此负责。

### §4 端口占用前置诊断（pp-client 启动流程）

`start_proxy` 生成配置后、spawn 核心前，探测 `mixed_port` 与 `clash_api_port`：

- 被外部进程占用 → 中文错误「端口 {port} 被占用（进程：{name}，PID {pid}），
  请关闭该进程或修改端口」；反查失败退化为「端口 {port} 已被其他程序占用」。
- 占用者反查 `pp-common::process::port_owner(port)`：Linux 解析
  `/proc/net/tcp{,6}` 找 socket inode → 扫 `/proc/*/fd` 反查 PID →
  `/proc/<pid>/comm` 取名；Windows `GetExtendedTcpTable` → PID → 进程名。
- 策略为报错并指明占用者（不做自动换端口：端口来源需保持静态可预期，
  UI / 连接追踪 / 诊断均依赖配置值）。

## 错误处理

- §1 挂接失败、§2 收割失败、§3 清理失败均降级为告警日志，不阻塞主流程；
- §4 是唯一面向用户的硬错误路径（启动前失败，无副作用）。

## 测试

- pp-common：guard spawn 可调用性（沿用 no_window 测试惯例）；pid 工具对
  `/bin/sleep` 替身进程的 alive/exe/kill 断言；端口反查对本地监听 socket 的断言。
- pp-core：pidfile 生命周期（start 写 / stop 删）、收割路径（含 exe 不匹配时
  不误杀的 PID 复用保护用例，替身进程为 `/bin/sleep` 实路径）。
- pp-client：端口占用检测单测（本地 `TcpListener` 模拟冲突）。
- §3 退出清理与 WSL 跨实例场景：手动验证记录（前端无测试运行器）。
- Windows 专属路径由 CI Windows 构建 + 手动验证覆盖（仓库既有惯例）。

## 文档同步

ADR-0012（本设计的决策记录）、`docs/architecture.md` §4.5、`CHANGELOG.md`。
