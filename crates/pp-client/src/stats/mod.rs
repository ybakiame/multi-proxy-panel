//! 客户端本地流量统计存储（SQLite）。
//!
//! 数据来源为 Clash API `/connections` 快照（WS 推送或 HTTP 轮询，见
//! [`crate::connections`]），按「域名/IP + 命中规则 + 出站节点」维度记账：
//!
//! - `daily_stats`：日聚合表，主键 `(date, target, destination_ip, rule,
//!   rule_payload, outbound)`，连接每次快照差分增量 upsert，长连接流量随快照
//!   逐步入账（而非关闭时一次性结算），"今日流量"实时可见；
//! - `conn_records`：关闭连接明细表（保留 7 天），供详情页检索。
//!
//! 存储文件为 `<data_dir>/stats.db`，schema 版本用 `PRAGMA user_version` 管理
//! （客户端轻量场景，不引入 sea-orm-migration）。

mod store;
#[cfg(test)]
mod tests;

pub use store::StatsStore;

use serde::{Deserialize, Serialize};

/// 明细表保留天数。
pub const RECORDS_RETENTION_DAYS: i64 = 7;
/// 日聚合表保留天数。
pub const DAILY_RETENTION_DAYS: i64 = 90;
/// 明细/聚合查询默认与最大返回条数。
pub const DEFAULT_QUERY_LIMIT: u32 = 200;
/// 查询返回条数上限。
pub const MAX_QUERY_LIMIT: u32 = 1000;

/// 被判定为「未代理」的出站 tag（小写比较）：直连 / 拦截。
pub const DIRECT_OUTBOUND_TAGS: [&str; 3] = ["direct", "block", "reject"];

/// 一次快照差分产生的一条增量记账（tracker → [`StatsStore`]）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatDelta {
    /// 聚合目标：域名（有 sniff/请求域名时）或目的 IP。
    pub target: String,
    /// 目的 IP（可与 target 同为 IP；域名场景下为该次连接实际解析结果）。
    pub destination_ip: String,
    /// 命中的规则类型（如 `DOMAIN-SUFFIX` / `RuleSet` / `MATCH`）。
    pub rule: String,
    /// 规则载荷（如命中的域名后缀 / 规则集 tag）。
    pub rule_payload: String,
    /// 叶子出站 tag（chains[0]，即最终出站；分组场景为组内选中的节点）。
    pub outbound: String,
    /// 本快照周期内新增的上行字节。
    pub upload: u64,
    /// 本快照周期内新增下行字节。
    pub download: u64,
    /// 是否为本周期内新出现的连接（用于 conn_count 计数）。
    pub new_conn: bool,
    /// 快照时间（Unix 秒）。
    pub seen_at: i64,
}

/// 日聚合行（`daily_stats` 表）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct DailyStatRow {
    /// 本地日期 `YYYY-MM-DD`。
    pub date: String,
    /// 聚合目标（域名或目的 IP）。
    pub target: String,
    /// 目的 IP。
    pub destination_ip: String,
    /// 命中规则类型。
    pub rule: String,
    /// 规则载荷。
    pub rule_payload: String,
    /// 叶子出站 tag。
    pub outbound: String,
    /// 当日累计上行字节。
    pub upload_bytes: i64,
    /// 当日累计下行字节。
    pub download_bytes: i64,
    /// 当日连接数。
    pub conn_count: i64,
    /// 最后一次见到该组合的时间（Unix 秒）。
    pub last_seen: i64,
}

/// 关闭连接明细行（`conn_records` 表）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::FromRow)]
pub struct ConnRecordRow {
    /// 自增主键。
    pub id: i64,
    /// 聚合目标（域名或目的 IP）。
    pub target: String,
    /// 展示用 host（域名或 `ip:port` 兜底，与 `ConnectionView.host` 一致）。
    pub host: String,
    /// 目的 IP。
    pub destination_ip: String,
    /// 网络协议（`tcp` / `udp`）。
    pub network: String,
    /// 完整出站链（展示用，如 `Proxy → DIRECT`）。
    pub chain: String,
    /// 叶子出站 tag。
    pub outbound: String,
    /// 命中规则类型。
    pub rule: String,
    /// 规则载荷。
    pub rule_payload: String,
    /// 上行字节（连接生命周期累计）。
    pub upload: i64,
    /// 下行字节（连接生命周期累计）。
    pub download: i64,
    /// 连接开始时间（Unix 秒）。
    pub started_at: i64,
    /// 连接关闭（从快照消失）时间（Unix 秒）。
    pub ended_at: i64,
}

/// 今日汇总（卡片数据源）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodaySummary {
    /// 本地日期 `YYYY-MM-DD`。
    pub date: String,
    /// 今日总上行字节。
    pub upload_bytes: i64,
    /// 今日总下行字节。
    pub download_bytes: i64,
    /// 今日已代理上行字节（排除 direct/block/reject 出站）。
    pub proxied_upload_bytes: i64,
    /// 今日已代理下行字节。
    pub proxied_download_bytes: i64,
    /// 今日连接数。
    pub connection_count: i64,
    /// 今日去重目标数（域名/IP）。
    pub target_count: i64,
}

/// 日聚合查询排序字段。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DailySort {
    /// 按上行字节。
    Upload,
    /// 按下行字节。
    Download,
    /// 按上下行合计（默认）。
    #[default]
    Total,
    /// 按连接数。
    Count,
    /// 按最后见到时间。
    LastSeen,
}

/// 明细查询排序字段。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordSort {
    /// 按上行字节。
    Upload,
    /// 按下行字节。
    Download,
    /// 按上下行合计。
    Total,
    /// 按开始时间。
    Started,
    /// 按关闭时间（默认）。
    #[default]
    Ended,
}

/// 日聚合查询条件。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DailyQuery {
    /// 本地日期 `YYYY-MM-DD`；缺省为今天。
    pub date: Option<String>,
    /// 模糊搜索（匹配 target / destination_ip / rule / rule_payload / outbound）。
    pub search: Option<String>,
    /// 仅看出站：精确匹配 outbound tag。
    pub outbound: Option<String>,
    /// 排序字段。
    pub sort: DailySort,
    /// 是否降序（默认 true）。
    pub desc: Option<bool>,
    /// 返回条数上限（默认 [`DEFAULT_QUERY_LIMIT`]，封顶 [`MAX_QUERY_LIMIT`]）。
    pub limit: Option<u32>,
}

/// 明细查询条件。
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RecordsQuery {
    /// 模糊搜索（匹配 target / host / destination_ip / rule / rule_payload / outbound）。
    pub search: Option<String>,
    /// 排序字段。
    pub sort: RecordSort,
    /// 是否降序（默认 true）。
    pub desc: Option<bool>,
    /// 返回条数上限（默认 [`DEFAULT_QUERY_LIMIT`]，封顶 [`MAX_QUERY_LIMIT`]）。
    pub limit: Option<u32>,
}

/// 将 Unix 秒格式化为本地日期 `YYYY-MM-DD`。
pub fn local_date(ts: i64) -> String {
    chrono::DateTime::from_timestamp(ts, 0)
        .map(|dt| {
            dt.with_timezone(&chrono::Local)
                .format("%Y-%m-%d")
                .to_string()
        })
        .unwrap_or_default()
}

/// 当前本地日期 `YYYY-MM-DD`。
pub fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// 判断出站是否为「未代理」（直连/拦截，大小写不敏感）。
pub fn is_direct_outbound(outbound: &str) -> bool {
    DIRECT_OUTBOUND_TAGS.contains(&outbound.to_ascii_lowercase().as_str())
}
