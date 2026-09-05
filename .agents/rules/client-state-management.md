# 客户端状态管理规则（apps/desktop）

> 解决问题：同一数据被多个页面/组件重复 invoke 读取（配置重复读、状态多处不一致）。

## 分层原则

| 层级 | 工具 | 存什么 | 示例 |
|------|------|--------|------|
| **服务端/命令状态**（Tauri invoke 的数据） | TanStack Query | 后端持有的数据，可缓存、可失效、可轮询 | config、proxy_status、subscriptions、proxies、connections、profiles、rules |
| **跨页共享原子状态**（纯前端、零散值） | jotai atom | 跨页/跨组件共享但没有领域动作的单个值 | 最近操作错误 `lastActionErrorAtom` |
| **跨页共享会话 store**（纯前端、带成组动作） | zustand store | 不来自后端、需要成组动作/订阅的会话状态 | toast 通知队列 |
| **局部 UI 状态** | useState | 单组件内的临时状态 | 弹窗开关、表单输入 |

选型判断：数据来自 invoke → Query；纯前端且要跨组件共享 → 零散的用 jotai，带领域动作成组出现的用 zustand；都不沾 → useState。**不确定时用更低的层级**，不要为了"以后可能共享"提前上 atom/store。

## 规则

1. **同一数据一个 queryKey，一处定义**：`queryKey` 常量集中定义于 `src/api/keys.ts`（含参数化 key 的工厂函数），多组件共享同一 key 自动去重读取，**禁止**在组件内各自写字面量 key 或各自 invoke 同一命令而不走 Query 缓存
2. **变更后回写或失效，不重读**：mutation 的 onSuccess 里优先 `setQueryData` 用返回值/入参回写（如 `useSaveConfig`、start/stop 回写 `PROXY_STATUS_KEY`），拿不到最新数据才 `invalidateQueries`；不要手动再 invoke 一遍塞 state
3. **轮询用 refetchInterval**：禁止 useEffect + setInterval + setState 的手动轮询
4. **loading 用 Query 状态**：`isLoading/isFetching/isPending` 替代手动 `setLoading(true/false)`（也顺带满足 React Compiler 对 try/finally 的限制）
5. **jotai atom 按域放置**：`src/atoms/<domain>.ts`，一个文件一个领域；atom 只存值，动作就近写在消费组件或 hook 里
6. **zustand store 按域拆分**：`src/` 下一个 store 一个职责（如 `toast.ts`）；不进 store 的数据绝不放
7. **禁止双写**：同一数据不要同时存在于 Query 缓存和 jotai/zustand/useState 中（择一权威源）；表单草稿类"加载自 Query、编辑在本地"的场景，用渲染期间 adjust-state 或 key 重挂载同步，不要 effect 里镜像

## 参考

- 现成模式：`src/hooks/useCapabilities.ts`（Query）、`src/hooks/useClientConfig.ts`（Query + 串行化 mutation）、`src/hooks/useProxyStatus.ts`（轮询）、`src/atoms/ui.ts`（jotai）、`src/toast.ts`（zustand）
- 规则联动：`.agents/rules/code-organization.md`
