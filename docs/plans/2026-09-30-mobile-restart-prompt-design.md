# 移动端全局配置变更重启提示 — 设计文档

日期：2026-09-30
范围：`apps/mobile`(UI)+ `packages/client-core`（脏标记 store，纯逻辑）
背景：核心运行中修改配置（DNS/出站/路由/规则等）不会热更新，目前仅靠 6 处保存成功 toast「重启代理后生效」提示，toast 即逝，用户容易忘记重启导致配置不生效。

## 交互形态（已确认）

**弹窗一次 + 悬浮按钮**：

1. 检测到需重启的脏配置且核心运行中 → 自动弹一次 Dialog（列出变更项），按钮「立即重启」/「稍后」；
2. 「稍后」→ 收为右下角悬浮按钮（启停 FAB 上方避让），点击开 BottomSheet：变更项列表 + 「立即重启」；
3. 「立即重启」→ `stopProxy` → `startProxy`，成功后清空脏标记；失败 toast 并保留脏标记；
4. 核心停止（手动停/异常退出）→ 脏标记与 dismissed 复位。

## 检测方式（已确认）

**保存动作上报**：各保存成功路径统一调用 `markRestartRequired(key)`（仅核心运行中才入队），核心停止/重启后清空。不覆盖 App 外改动（移动端无此场景）。

## 脏标记 store(`@pp/client-core`)

zustand store（desktop 后续可复用同逻辑自渲染 UI):

```ts
interface PendingRestartStore {
  /** 脏顶级配置域：key = 域 id，value = 中文展示名。 */
  dirtyKeys: ReadonlyMap<string, string>;
  /** 本次脏周期内自动弹窗是否已被「稍后」收起。 */
  dismissed: boolean;
  markDirty: (key: RestartDirtyKey) => void;
  dismiss: () => void;
  /** 核心停止/重启成功后复位。 */
  reset: () => void;
}
```

`RestartDirtyKey` 联合类型：`dns | outbounds | route | experimental | rules | rulesets | inbounds | clash_api | subscription`，标签映射常量同模块导出。

## 上报点映射

| 保存路径 | key |
|---|---|
| `configSlicesSave`(DNS 页） | `dns` |
| `configSlicesSave`（出站页） | `outbounds` |
| `configSlicesSave`（路由页） | `route` |
| `configSlicesSave`（实验页） | `experimental` |
| `localOverrideSave`（规则管理） | `rules` |
| `localOverrideSave`（规则集） | `rulesets` |
| 即时保存 `mixed_port` / `tun_stack` / `tun_auto_route` / `ipv6_enabled` | `inbounds` |
| 即时保存 `clash_api_*` | `clash_api` |
| 即时保存 `dns_fakeip_enabled` | `dns` |
| 切换生效订阅 | `subscription` |

不上报：`rule_mode`（即时生效）、`vpn_notify_*`（热更新通知栏）、`github_proxy_prefix` / `fetch_via_local_proxy`（不影响运行中核心）。

## 组件（mobile)

`components/RestartPrompt.tsx`，挂 `App.tsx` 根部：

- 消费 store + `useProxyStatus()`;`dirtyKeys` 非空 && 运行中 && 未 dismissed → `ConfirmDialog`（标题「配置已变更」，内容列变更项标签，确认「立即重启」/ 取消「稍后」→ dismiss);
- dismissed 后渲染悬浮按钮（Konsta `Fab` 迷你形态或圆角小按钮，右下角，`bottom` 避让启停 FAB 与悬浮 TabBar)，点击开 `BottomSheet`：变更项列表 + 「立即重启」;
- 重启链路：`stopProxy` → `startProxy`（已授权过不再弹 VPN 授权；若遇 `vpn_not_authorized` 则 toast 引导），进行中禁用防重复，成功 `reset()` + toast「代理已重启，配置已生效」;
- `coreRunning` 转 false 时 `reset()`（用 effect 监听 status 变化）。

## 错误处理

- stop/start 失败 → `toastError` + 保留脏标记（用户可再次发起）;
- 重启期间 Dialog/BottomSheet 按钮禁用并显示 pending。

## 测试

- 手动（adb 真机）：运行核心 → 改 DNS 设置保存 → 弹窗出现 → 「稍后」→ 悬浮按钮 → BottomSheet 列「DNS 管理」→ 「立即重启」→ toast 成功 → 悬浮按钮消失；
- 核心未运行时保存 → 不出现任何提示；
- 手动停止核心 → 提示消失。
