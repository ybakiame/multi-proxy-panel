# ADR-0014: Windows 自定义窗口控件与双端代理策略

- **Status:** Accepted
- **Date:** 2026-10-11
- **Deciders:** 项目维护者（本轮需求）
- **Related:** [ADR-0007](0007-client-single-shell-merge.md)、[ADR-0013](0013-client-own-ui-library.md)

## Context

Windows 系统标题栏与客户端主题不协调；移动端内嵌 Clash Web UI 与共享页面的
交互不一致。双端页面树已合一，应继续复用现有 Rust 核心与共享 UI。

## Decision

Windows 壳在 setup 阶段关闭系统装饰；共享 Shell 提供标题栏插槽，客户端按
capabilities.os 显示 Windows 控件。窗口 API 经 client-core 封装，权限仅授予
Windows 主窗口。关闭仍调用系统窗口 close，沿用现有退出与核心清理流程。
Linux/macOS 保留原生装饰，Android 不显示窗口控件。

双端 `/proxies` 使用同一策略页，通过已有共享命令查询、切换和测速；移动端
首页进入该页。移除内嵌 Clash 面板实现，旧 `/panel` 地址重定向到策略页；
Clash API 继续作为核心控制接口，现有外部 UI 配置不改变。

MITM 的 CA 读取、系统信任查询与安装由异步命令调度到 blocking 线程池，
不得在窗口线程执行系统命令或证书 IO。前端缓存信任状态五分钟，提供手动刷新。

## Consequences

继续维护一套双端策略交互；Windows 标题栏需要真机验证拖动、最大化、还原、
缩放和关闭清理。关于产品与移动核心分卡；磁盘日志以类型/文件选择器和内容
组成一张卡，桌面实时缓冲保留独立语义。
