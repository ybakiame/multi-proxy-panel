# 桌面端同步移动端能力 + Windows 安装编译

日期：2026-10-01
范围：`apps/desktop`、`packages/client-core`、`crates/pp-client`、`crates/pp-mitm`、`.github/workflows`、`docs/`

## 目标

1. **分段配置**：把移动端 Config 结构化切片编辑页（DNS / 出站 / 路由 / 实验性）完整移植到桌面端（HeroUI 重写，复用已共享的 client-core `configSlices` API 与 Rust 后端）
2. **全局重启提示**：桌面端接入 client-core `pendingRestart` store，实现 RestartPrompt（弹窗一次 + 悬浮按钮）
3. **其他优化（本次圈定三项）**：主题切换（浅/深/跟随系统）、规则页物化模型 UI 补齐（内置徽章/禁删/还原入口）
4. **Windows**：修通编译、TUN 管理员提权链路、CI 出 NSIS 安装包

## 现状与关键事实

- 后端已全部共享：`pp-client` 的 config_slices、物化模型、规则/规则集后端，桌面壳已注册 `config_slices_get/save`、`builtin_dns_slice_get`、`dns_server_probe` 等命令——**只缺桌面前端**
- `pendingRestart` store 已在 `@pp/client-core`（移动端已用，桌面端零上报点）
- 桌面端已有点位：Rules 页（缺物化模型 UI）、Settings（NetworkSettings/ClashPanelSettings 已有 mixed_port/tun_stack/clash_api 字段，缺 dirty 上报）、Dashboard（订阅切换，缺 dirty 上报）、无 FakeIP UI、ThemeBootstrap 强制深色
- Windows 基础较好：核心下载已支持 zip/.exe、`sysproxy`/`privilege` 已有 windows 分支（TUN 是 stub）；缺口：`pp-mitm/src/ca.rs:9` 顶层 `std::os::unix` 导入未加 cfg、`tauri.conf.json` `bundle.active=false` 且无图标、CI 无 Windows job、无 wintun.dll 处理

## 改动分解（按原子提交序）

### A. 桌面端分段配置 UI（主体工作，约 6-8 个提交）

新增 `apps/desktop/src/pages/Config/`，路由挂在现有侧边栏：

- `/config` 入口页（卡片列表，对齐 mobile `Config/index.tsx`）
- `/config/dns`：DNS 服务器列表（预设目录选择、探测延迟、删除确认）+ 规则列表/编辑 Modal + FakeIP 卡片 + 内置默认可编辑语义（`builtinDnsSliceGet` 初始化草稿，保存即物化为 takeover）；Sheet→Modal 改写
- `/config/outbounds`：自定义出站 + 内置分组（proxy/auto）物化 UI（字段覆写、可改不可删、徽章）、协议表单（vless/vmess/ss/trojan/hy2/selector/urltest）
- `/config/route`：final 出站 + 默认域名解析器设置
- `/config/experimental`：cache_file 设置
- 侧边栏导航（`layout/config/nav/desktop.ts`）加「配置」项；App.tsx 注册路由
- 入站/Clash API **不重复造页**：桌面设置页已有等价卡片，仅补 dirty 上报
- 所有保存成功路径接 `markRestartRequired(domain, coreRunning)`

### B. 全局重启提示（1 个提交）

- 新增 `apps/desktop/src/components/RestartPrompt.tsx`：HeroUI Modal（列变更项，「立即重启」/「稍后」）+ 右下角悬浮按钮（避让主内容，fixed 定位）+ HeroUI Modal 变更清单；挂 `AppContent` 根部
- 重启链路 `stopProxy → 轮询 proxy_status 确认停止 → startProxy`，成功 reset+toast，失败保留脏标记；核心转停止自动复位（照搬 mobile 逻辑，去掉 VPN 授权分支）
- 补上报点：Rules 页 persist（rules/rulesets）、Settings `useSettingsConfig`（inbounds/clash_api）、Dashboard 订阅切换（subscription）

### C. 规则页物化模型 UI 补齐（1-2 个提交）

- desktop Rules 页：内置规则徽章、禁删/限字段编辑、「还原内置规则（置顶）」按钮（按基线视图补回缺失内置规则，参考 mobile `CustomRulesPage` 的 restore 实现）
- 规则集区：内置徽章 + 还原入口（参考 `RuleSetsPage`）、`localOverrideGuards` 守卫逻辑移植

### D. 主题切换（1 个提交）

- `apps/desktop/index.html` 去掉静态 `class="dark" data-theme="dark"`
- ThemeBootstrap 改为读取偏好（light/dark/system）：偏好持久化 localStorage（新键 `pp:theme`，桌面无历史键无需迁移），system 用 `matchMedia` 监听
- Settings 新增外观卡片（三选），文案对齐移动端

### E. Windows 编译修通（1-2 个提交）

- `pp-mitm/src/ca.rs`：unix import 加 `#[cfg(unix)]` 门控，Windows 写入路径用普通 `fs::write`（ACL 收紧后续再议）
- 对 `pp-client` / `pp-client-tauri` / desktop 壳跑 `cargo check --target x86_64-pc-windows-msvc` 扫雷并逐个修复（预计还有零星 cfg 缺口）
- 已知 gated 良好：WSL workaround（cfg linux）、privilege（全平台分支齐）、核心下载（zip/.exe）

### F. Windows TUN 提权（1-2 个提交）

- `pp-client/src/privilege.rs` Windows 实现：
  - `tun_auth_status`：检测当前进程管理员令牌（新增 `[target.'cfg(windows)'.dependencies] windows-sys`，`CheckTokenMembership` 查 Administrators 组；`windows-sys` 仅 cfg(windows) 引入，不影响 Linux/mac 构建）
  - `authorize_tun`：`ShellExecuteW`（verb `runas`）以管理员身份重启应用自身，当前进程退出
- wintun.dll：sing-box Windows TUN 依赖 `wintun.dll` 与 exe 同目录（实现时先验证 sing-box 1.15 是否内嵌；若未内嵌，核心下载完成后 Windows 上自动从 wintun.net 官方 zip 拉取对应架构 dll 落核心目录）
- 前端 Settings TUN 授权卡片补 Windows 文案（「以管理员身份重启」）

### G. NSIS 安装包 + CI（1-2 个提交）

- `apps/desktop/src-tauri/tauri.conf.json`：`bundle.active=true`、`targets:["nsis"]`、补齐图标（`bunx tauri icon` 从现有 `icons/icon.png` 生成全套含 .ico 并提交）
- `.github/workflows/release.yml` 新增 `desktop-windows` job：windows-latest → bun install → 前端 build → `tauri-apps/tauri-action` 构建 → NSIS `.exe` 上传 release
- 不做桌面 Linux/macOS 打包与自动更新（范围外）

### H. 文档（1 个提交）

- `docs/development.md`：Windows 构建/打包章节（前置依赖、NSIS 产物位置、TUN 权限说明）
- `README.md`：平台支持矩阵加 Windows
- `docs/plans/` 留设计/决策记录（Windows 提权与 wintun 结论）

## 验证

- `bun run --filter pp-client-ui verify`、`bun run --filter @pp/client-core verify`（lint+fmt+build）
- `bun run verify:rust`（含双壳 clippy）
- 本机 `cargo check --target x86_64-pc-windows-msvc`（check 不需链接器，可拦截绝大多数 cfg/平台编译错误）
- CI windows job 绿 + release 产物含 NSIS 安装包
- **环境限制声明**：本机为 Linux，无 Windows 真机；NSIS 安装包实际安装、TUN 提权交互、SmartScreen 行为需 Windows 环境手动验证，方案会留出可回滚的单测与日志

## 主要风险

- Windows 编译可能有 CI 才能暴露的链式问题（tauri-action 构建迭代）
- wintun.dll 是否需随核心分发依赖 sing-box 版本行为，实现时先验证再决定是否加下载逻辑
- NSIS 未签名会触发 SmartScreen 警告（无代码签名证书，文档说明即可）
- 移动端 Config 页约 5900 行 Konsta UI，桌面 HeroUI 重写会合并部分组件（Sheet→Modal、列表区合并），目标控制在 ~3500 行内，逐页原子提交
