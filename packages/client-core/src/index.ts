/**
 * @pp/client-core 包根统一导出。
 *
 * Desktop / Mobile 双端共享的前端核心库：Tauri invoke api 封装 + query keys +
 * hooks + atoms + 纯工具（toast / logCapture / env）。
 *
 * 消费方一律 `import { ... } from "@pp/client-core"`（统一包根导入，不使用子路径）；
 * 本包内文件互相引用保持相对路径。
 */

// API 封装（types + functions）与 query keys 集中定义。
export * from "./api";
export * from "./api/keys";

// Jotai 共享状态。
export * from "./atoms/ui";

// React Query 数据 hooks。
export * from "./hooks/useCapabilities";
export * from "./hooks/useClientConfig";
export * from "./hooks/useCoreVersion";
export * from "./hooks/useDnsServers";
export * from "./hooks/useProxyStatus";
export * from "./hooks/useSettingsPersist";
export * from "./hooks/useRuleSetMarket";

// 纯逻辑工具。
export * from "./coreChannels";
export * from "./dnsSlice";
export * from "./dnsValidate";
export * from "./env";
export * from "./localOverrideGuards";
export * from "./logCapture";
export * from "./metaCubeCatalog";
export * from "./outboundForm";
export * from "./outboundOptions";
export * from "./groupForm";
export * from "./pendingRestart";
export * from "./ruleSetOptions";
export * from "./routeSlice";
export * from "./rules";
export * from "./theme";
export * from "./toast";
