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
export * from "./hooks/useProxyStatus";

// 纯逻辑工具。
export * from "./env";
export * from "./logCapture";
export * from "./rules";
export * from "./toast";
