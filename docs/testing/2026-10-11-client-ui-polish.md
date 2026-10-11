# 客户端窗口与共享信息布局回归

日期：2026-10-11；决策：[ADR-0014](../adr/0014-client-window-and-proxy-ui.md)。

## 实现

- Windows 自定义标题栏：最小化、最大化/还原、关闭；拖动区域与控件分离。
  Shell 使用独立标题栏插槽，内容在剩余高度内滚动，Linux/macOS 保留原生装饰。
- 产品卡共用产品名称、描述、壳版本、项目链接与检查更新。移动端核心版本和编译
  tags 单独成卡；移动检查更新通过系统浏览器打开发布页，不调用桌面 updater。
- 双端磁盘日志合为“日志记录”：类型选择（应用/核心/启动快照）、文件选择、
  内容。文件内容由独立 Query key 管理，快速切换不会被旧请求覆盖；刷新后移除
  不存在的选择。桌面实时内存缓冲、导出、清空保留原语义。
- 首页双端复用 ProxyStrategyCard：路由模式、当前出口、策略组入口。移动 `/panel`
  重定向到共享 `/proxies`；移除 iframe 页面与策略页管理订阅按钮。
  节点选择和测速使用并列按钮；类型标签与延迟格式共用。

## 验证

全部编译、运行与浏览器工具在项目 Nix 开发环境执行。

- pp-client-app、@pp/client-core、@pp/ui verify 通过。
- cargo build --workspace 与完整 verify:rust 通过（含壳 host 与 Android target）。
- agent-browser 浏览器 mock：桌面应用日志下拉仅列出 app.log 文件；选择文件后
  显示对应正文。390×844 移动端切换核心日志仅列 libbox.log，正文正确且无横向溢出。
- 移动关于页包含独立产品卡与核心卡，版本 0.1.1、项目链接及检查更新按钮可见。
- 双端策略页：Selector 可切换、URLTest 只读；单节点测速只调用 test_delay，
  不调用 select；选择节点调用对应 group/name，并更新选中出口。无嵌套按钮，
  无管理订阅按钮，无 iframe。移动深色布局检查通过，首页停机时仍可设置策略模式。
- Windows capability mock：显示三个窗口控件，最大化按钮调用 main 窗口的
  toggle_maximize。标题栏与内容滚动布局检查通过。
- 延迟十秒的信任查询 mock：查询中可在 500ms 内导航并显示设置页。

## 真机验收边界

浏览器 mock 不证明系统窗口行为。Windows 测试构建需真机确认拖动/双击标题栏、
最小化、最大化/还原、边缘缩放、DPI、关闭后的核心清理，以及 certutil 查询中
窗口仍可操作。Android UI 后续通过 Nix 内 tauri android dev 热更新验证，不覆盖
安装设备上存在未保存草稿的应用。
