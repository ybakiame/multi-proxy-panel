# 客户端 UI 迁移验证（2026-10-10）

范围：ADR-0013 M3–M6 的组件迁移、统一入口/路由与逐页收敛。仪表盘保留计划允许的
平台控制布局；运行数据、启停回写与通用控件共用。无 Rust 修改。

## 自动验证

| 命令 | 结果 |
| --- | --- |
| `bun run --filter @pp/ui verify` | 通过：类型、lint、格式 |
| `bun run --filter pp-client-app verify` | 通过：桌面/Android 两套构建、lint、格式 |
| `bun run --filter @pp/client-core verify` | 通过：类型、lint、格式（共享仪表盘模型与 toast 说明） |
| `git diff --check` | 通过 |

源码与客户端依赖清单无 HeroUI/Konsta 导入；`@app` alias 已移除。Android 构建产物检查未
发现桌面 `plugin:updater`、`mitm_status`、`remote_add` 等专属命令。业务源文件无超过
500 行的新增文件。前端无单元测试运行器，采用上述 verify 与浏览器交互回归。

## 浏览器交互回归

使用 dev 的 `?browser-debug=1` mock IPC，桌面 1420、移动 1430（390 × 844）。规则保存
夹具仅注入隔离浏览器会话，不写真实客户端配置，不进入产品代码。

| 检查 | 结果 |
| --- | --- |
| 桌面规则编辑 | 居中 Dialog；初始选择显示中文标签；匹配候选包含进程名 |
| 移动规则编辑 | BottomSheet；嵌套匹配选择抽屉包含应用包名；选择后返回规则表单 |
| 保存失败 | 错误提示；表单仍打开，目标和序列化参数保留 |
| 保存重试 | 成功关闭表单；移动出现规则卡片，桌面出现规则表格行 |
| 再次打开新增 | 上一条规则目标已清空 |
| 桌面 Esc | 关闭弹窗，焦点返回「添加规则」 |
| 连续 Tab（各 12 次） | 桌面 Dialog 与移动 Drawer 的焦点均保持在弹层内 |
| Material 风格 | 即时更新 `<html data-ui-style="material">` 与背景令牌；`pp-ui-style` 持久化 |
| 桌面订阅导航 | 点击侧栏订阅后 URL 为 `/subscriptions`，订阅链接保持 `aria-current="page"` 与高亮样式；内部入口不再使用 `/nodes` |
| 旧路由 | 桌面 `/nodes` → `/subscriptions`；移动 `/config/rules` → `/config/route/rules` |

axe-core 4.12.1（wcag2a/wcag2aa）：桌面规则表格、移动规则卡片的浅色/深色检查均为
**0 violations**；规则弹层也检查了命名、焦点与嵌套状态。Base UI 的隐藏背景/焦点守卫
会出现 `aria-hidden-focus` 的 incomplete 项，已补充上述键盘焦点验证，不把 incomplete
误报为自动全绿。检查发现的通知区域 ARIA、提示文字/强调色对比度和按钮换行已修复。

## 边界与后续

未运行真实 Tauri WebView、Android 设备上的 VPN 授权/Drawer 手势，亦未对所有页面做
真实后端数据回归。本次不改命令接口、配置合成或壳层；仪表盘的 VPN/桌面控制布局
按已接受计划保留。Material 波纹仍按 ADR 后置。

## 日志页合一回归

`pp-client-app verify` 通过。隔离浏览器夹具验证桌面与 390×844 移动视口：
文件读取以 `name` 与 `maxLines=1000` 调用共享 API；移动未调用 `get_logs`；
导出展示返回路径、读取失败展示错误；桌面实时缓冲与文件预览并存，清空弹窗确认后
重新请求运行日志，列表变为空态。仍未覆盖真实磁盘导出与系统剪贴板权限。

## 统计页合一回归

`pp-client-app verify` 通过。相同样例在桌面生成语义表格、移动生成卡片列表且无横向溢出；
点击桌面「上行」表头产生 `sort=upload, desc=true` 查询；切换连接明细后桌面保留
网络/链路字段与 `sort=started` 默认值，移动保持 `sort=total, limit=500`。
本次未执行真实统计清空；确认弹窗与失效三类缓存的原流程保持。

## 入站页合一回归

`pp-client-app verify` 通过。模拟配置在双端回显端口 17890、mixed 与自动路由；
桌面 TUN 启用时查询授权，模拟授权后显示已授权；移动没有 TUN 关闭开关，也没有
桌面授权查询。输入非法端口 0 显示范围错误，未调用保存。真实系统提权仍待设备验证。

## 设置页收敛回归

`pp-client-app verify` 通过。浏览器检查两端设置主页各只有一个标题与外观卡；
桌面保留 GitHub 代理测试、核心管理和检查更新，移动保留 VPN 通知/GitHub/开发工具/
关于二级入口。真实核心下载、更新安装与代理连通性未在浏览器 mock 中执行。

## 订阅页合一回归

`pp-client-app verify` 通过。同一订阅样例在桌面显示模板关联/预览表格，移动显示
生效卡片；共享表单桌面预填覆写甲和 sing-box UA，移动编辑保留未展示的 p1 绑定。
模拟保存失败时两端表单保持打开，`update_subscription` 参数均包含原 profileId=p1。
真实订阅拉取与配置预览合成未在 mock 中执行。

## 仪表盘收敛回归

`pp-client-app verify` 与 `@pp/client-core verify` 通过。模拟生效订阅与核心后，桌面
启动一次成功回写运行状态；移动首次启动返回 `vpn_not_authorized`，按启动→请求授权→
再启动顺序成功，两端随后停止成功。今日流量卡可通过按钮语义进入统计详情；规则模式
控件共用，成功回写状态并失效配置缓存。桌面未调用 VPN 授权接口。

## 移动抽屉键盘避让回归

`@pp/ui verify` 与 `pp-client-app verify` 通过。390×844 浏览器视口模拟
VisualViewport 高度由 844 缩至 320：抽屉底部偏移 524px、最大高度 304px，
聚焦 User-Agent 后内部滚动 96px，输入框底部为 226px，处于键盘上方。
恢复 844 后偏移归零。自动化仅覆盖视口变化与焦点滚动，真实 Android 输入法待设备验收。

## 订阅开关移除与 URL 回归

在项目 `nix develop` 环境执行 `pp-client-app verify`、`@pp/client-core verify` 通过。
双端订阅页无启用开关/列，桌面与移动首页候选包含旧 `enabled=false` 条目；选择时
依次调用 `set_subscription_enabled(enabled=true)`、`set_active_subscription`。
移动 600 字符 URL 的 clientWidth=330、scrollWidth=4537、textOverflow=ellipsis；
390px 视口内 document.scrollWidth=390，无横向溢出。存储字段保留，未执行启动迁移。
