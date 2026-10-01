/**
 * Traffic statistics API types and functions.
 *
 * Aligned with Rust-side `TodaySummary`, `DailyStatRow`, `ConnRecordRow`,
 * `DailyQuery`, `RecordsQuery`（pp-client `stats` 模块）。
 */

import { invoke } from "@tauri-apps/api/core";

/** 今日流量汇总（首页卡片数据源）。 */
export interface TodaySummary {
  /** 本地日期 `YYYY-MM-DD`。 */
  date: string;
  /** 今日总上行字节。 */
  upload_bytes: number;
  /** 今日总下行字节。 */
  download_bytes: number;
  /** 今日已代理上行字节（排除 direct/block/reject 出站）。 */
  proxied_upload_bytes: number;
  /** 今日已代理下行字节。 */
  proxied_download_bytes: number;
  /** 今日连接数。 */
  connection_count: number;
  /** 今日去重目标数（域名/IP）。 */
  target_count: number;
}

/** 日聚合行（按 日期 + 域名/IP + 规则 + 出站 维度）。 */
export interface DailyStatRow {
  date: string;
  /** 聚合目标（域名或目的 IP）。 */
  target: string;
  destination_ip: string;
  /** 命中规则类型（如 `DOMAIN-SUFFIX` / `RuleSet` / `MATCH`）。 */
  rule: string;
  /** 规则载荷。 */
  rule_payload: string;
  /** 叶子出站 tag（最终出站节点）。 */
  outbound: string;
  upload_bytes: number;
  download_bytes: number;
  conn_count: number;
  /** 最后见到时间（Unix 秒）。 */
  last_seen: number;
}

/** 关闭连接明细行。 */
export interface ConnRecordRow {
  id: number;
  target: string;
  host: string;
  destination_ip: string;
  network: string;
  chain: string;
  outbound: string;
  rule: string;
  rule_payload: string;
  upload: number;
  download: number;
  started_at: number;
  ended_at: number;
}

/** 日聚合查询排序字段。 */
export type DailySort = "upload" | "download" | "total" | "count" | "last_seen";

/** 明细查询排序字段。 */
export type RecordSort = "upload" | "download" | "total" | "started" | "ended";

/** 日聚合查询条件（缺省日期为今天）。 */
export interface DailyQuery {
  date?: string;
  /** 模糊搜索（target / destination_ip / rule / rule_payload / outbound）。 */
  search?: string;
  /** 精确过滤出站 tag。 */
  outbound?: string;
  sort?: DailySort;
  /** 默认 true。 */
  desc?: boolean;
  limit?: number;
}

/** 明细查询条件。 */
export interface RecordsQuery {
  /** 模糊搜索（target / host / destination_ip / rule / rule_payload / outbound）。 */
  search?: string;
  sort?: RecordSort;
  /** 默认 true。 */
  desc?: boolean;
  limit?: number;
}

/** 今日流量汇总。 */
export function statsToday(): Promise<TodaySummary> {
  return invoke<TodaySummary>("stats_today");
}

/** 日聚合查询。 */
export function statsDaily(query: DailyQuery = {}): Promise<DailyStatRow[]> {
  return invoke<DailyStatRow[]>("stats_daily", { query });
}

/** 关闭连接明细查询。 */
export function statsRecords(query: RecordsQuery = {}): Promise<ConnRecordRow[]> {
  return invoke<ConnRecordRow[]>("stats_records", { query });
}

/** 清空全部统计数据（聚合 + 明细）。 */
export function statsClear(): Promise<void> {
  return invoke<void>("stats_clear");
}
