/**
 * 连通性诊断 API（开发者工具）。
 *
 * Aligned with Rust `pp_client::diagnose`（`DiagReport` / `DiagStep` / `DiagStatus`）。
 * 对单个域名沿流量路径分层检查：系统 DNS / 直连 TCP / 各生效 DNS 服务器真实查询 /
 * 核心状态（含降级）/ 经核心 mixed 入站全链路 / 主分组出站延迟 / TUN 路径说明。
 */

import { invoke } from "@tauri-apps/api/core";

/** 诊断步骤状态。 */
export type DiagStatus = "ok" | "fail" | "skip" | "info";

/** 单步诊断结果。 */
export interface DiagStep {
  /** 稳定标识（`system-dns` / `core-mixed` 等）。 */
  key: string;
  /** 展示名。 */
  name: string;
  status: DiagStatus;
  /** 步骤耗时（毫秒）。 */
  duration_ms: number;
  /** 一行结论。 */
  summary: string;
  /** 详细内容（多行）。 */
  detail: string;
}

/** 诊断报告。 */
export interface DiagReport {
  domain: string;
  steps: DiagStep[];
  /** 失败步骤数（ok/fail 口径）。 */
  failed_steps: number;
}

/** 运行连通性诊断（`domain` 缺省 google.com）。 */
export function diagnoseConnectivityRun(domain?: string): Promise<DiagReport> {
  return invoke<DiagReport>("diagnose_connectivity_run", { input: { domain: domain ?? null } });
}
