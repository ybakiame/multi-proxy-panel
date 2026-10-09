# ADR-0012: 客户端核心进程生命周期管理（纵深防御）

- **Status:** Accepted
- **Date:** 2026-10-09
- **Deciders:** ProxyPanel Contributors
- **Scope:** `crates/pp-common`（process）、`crates/pp-core`（manager）、`crates/pp-client`（启动流程）、`apps/client/src-tauri`（退出清理）
- **Related:** ADR-0007（单壳双目标）、ADR-0010（内嵌 pinned 核心）、[设计文档](../plans/2026-10-09-client-core-lifecycle-design.md)

---

## 1. Context

桌面端核心（sing-box）作为子进程 spawn，进程句柄仅存于内存。实证故障：应用在
WSL 中冻结被强关后无任何清理，孤儿 sing-box 继续持有 `127.0.0.1:9090`，经 WSL
localhost 转发与 Windows 侧新实例冲突，用户只能看到 sing-box 原始 FATAL 日志。

生命周期缺陷是**四层叠加**的：无 OS 级父子绑定（父进程被杀核心必成孤儿）、无
PID 文件/启动收割（重启后对遗留核心无感知）、无退出事件清理（关窗不停核心）、
端口冲突无诊断（原始错误直抛）。只修任何单层都会在其他层留下可复现的漏洞
（例如只做退出清理无法覆盖"冻结被杀"——冻结进程根本收不到退出事件）。

## 2. Decision

采用**纵深防御**四层方案（详见设计文档）：

- **D1 OS 级父子绑定**：Windows Job Object（KILL_ON_JOB_CLOSE）+ Linux
  `PR_SET_PDEATHSIG`，父进程以任何方式死亡核心即被 OS 回收；macOS 无等价机制，
  显式放弃并由 D2 兜底。
- **D2 PID 文件 + 启动收割**：`<config_dir>/sing-box.pid` 记录 pid/exe/启动时间，
  `start()` 前校验 exe 路径一致后收割遗留进程（防 PID 复用误杀）。
- **D3 退出清理**：桌面壳 `RunEvent::ExitRequested` best-effort 停止核心 +
  恢复系统代理；只负责正常退出路径。
- **D4 端口占用前置诊断**：`start_proxy` spawn 前探测 `mixed_port` /
  `clash_api_port`，被外部进程占用时报中文错误并指明占用者进程名与 PID。

端口冲突策略为**报错而非自动换端口**：端口来源保持静态可预期（UI、连接追踪、
诊断均依赖配置值），动态端口引入的状态同步复杂度不符合 YAGNI。

## 3. Consequences

- 父进程被 SIGKILL/结束任务/崩溃时核心不再存活（Windows/Linux/WSL），孤儿
  场景收敛为"旧版本遗留 + macOS + OS 绑定未生效边角"，由 D2 收割。
- 跨 OS 实例冲突（孤儿在 WSL、新实例在 Windows）无法靠收割解决，由 D4 提供
  可操作的诊断信息。
- pp-common::process 引入 target 门控的 `windows-sys` / `libc` 依赖与少量
  unsafe（Job Object / pre_exec / GetExtendedTcpTable），unsafe 面集中于单模块，
  由单测 + Windows CI 构建覆盖。
- 所有清理/收割失败降级为告警，唯一的硬错误路径是 D4（启动前、无副作用）。

## 4. Alternatives considered

- **仅 PID 收割 + 退出清理（方案 B）**：实现更简单，但父进程被杀时仍产生孤儿，
  恰是实证故障的触发路径，弃。
- **仅端口诊断（方案 C）**：不解决孤儿问题，弃。
- **端口被占自动换空闲端口**：体验顺滑但端口来源动态化，UI/连接追踪/诊断的
  状态同步复杂度高，不符合 YAGNI，弃。
