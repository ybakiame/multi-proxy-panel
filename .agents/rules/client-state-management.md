# 客户端状态管理规则（apps/desktop）

> 解决问题：同一数据被多个页面/组件重复 invoke 读取（配置重复读、状态多处不一致）。

## 分层原则

| 层级 | 工具 | 存什么 | 示例 |
|------|------|--------|------|
| **服务端/命令状态**（Tauri invoke 的数据） | TanStack Query | 后端持有的数据，可缓存、可失效、可轮询 | config、proxy_status、subscriptions、proxies、connections、profiles、rules |
| **共享 UI 状态**（纯前端、跨页面/组件共享） | zustand store | 不来自后端的会话级 UI 状态 | 全局 busy、当前编辑草稿、跨页共享的 UI 偏好 |
| **局部 UI 状态** | useState | 单组件内的临时状态 | 弹窗开关、表单输入 |

## 规则

1. **同一数据一个 queryKey，一处定义**：`queryKey` 常量集中定义（如 `src/api/keys.ts`），多组件共享同一 key 自动去重读取，**禁止**在不同组件各自 invoke 同一命令而不走 Query 缓存
2. **变更后失效而非重读**：mutation 的 onSuccess 里 `invalidateQueries`，不要手动再 invoke 一遍塞 state
3. **轮询用 refetchInterval**：禁止 useEffect + setInterval + setState 的手动轮询
4. **loading 用 Query 状态**：`isLoading/isFetching` 替代手动 `setLoading(true/false)`（也顺带满足 React Compiler 对 try/finally 的限制）
5. **zustand store 按域拆分**：`src/stores/<domain>.ts`，一个 store 一个职责；不进 store 的数据绝不放
6. **禁止双写**：同一数据不要同时存在于 Query 缓存和 zustand/usetState 中（择一权威源）

## 参考

- 现成模式：`src/hooks/useCapabilities.ts`
- 规则联动：`.agents/rules/code-organization.md`
