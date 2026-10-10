# 客户端 UI 迁移验证（2026-10-10）

范围：ADR-0013 M3/M4、M5 的共享配置页面与统一入口/路由。M5 尚有页内平台实现待收敛，
不将本记录作为所有页面逻辑完全去重的证明。无 Rust 修改。

## 自动验证

| 命令 | 结果 |
| --- | --- |
| `bun run --filter @pp/ui verify` | 通过：类型、lint、格式 |
| `bun run --filter pp-client-app verify` | 通过：桌面/Android 两套构建、lint、格式 |
| `bun run --filter @pp/client-core verify` | 通过：类型、lint、格式（本次仅更新 toast 说明） |
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
真实后端数据回归。本次不改命令接口、配置合成或壳层。仪表盘、订阅、统计、日志、设置、
入站仍保留页内平台实现；后续按页继续去重并做真实设备验证。Material 波纹仍按 ADR
后置。
