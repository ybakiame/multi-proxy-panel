//! SQLite 存储实现（`stats.db`）。

use std::path::Path;
use std::time::Duration;

use pp_common::{PanelError, PanelResult};
use sqlx::Row;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};

use crate::connections::ConnectionView;
use crate::stats::{
    ConnRecordRow, DAILY_RETENTION_DAYS, DEFAULT_QUERY_LIMIT, DailyQuery, DailySort, DailyStatRow,
    MAX_QUERY_LIMIT, RECORDS_RETENTION_DAYS, RecordSort, RecordsQuery, StatDelta, TodaySummary,
};

/// 统计存储数据库文件名（位于客户端数据目录下）。
pub const STATS_DB_FILE: &str = "stats.db";

/// 当前 schema 版本（`PRAGMA user_version`）。
const SCHEMA_VERSION: i64 = 1;

// SQL 中直连出站 tag 字面量列表必须与 `DIRECT_OUTBOUND_TAGS` 保持一致（小写）。
const DIRECT_TAG_SQL_LIST: &str = "'direct','block','reject'";

/// 客户端流量统计存储（SQLite，WAL 模式）。
///
/// 写入路径为 tracker 快照差分（[`Self::apply_batch`]），读路径为 Tauri 命令
/// 层的今日汇总 / 聚合查询 / 明细查询。池固定单连接：本场景写入为秒级小批量，
/// 单连接串行足够，也规避了 SQLite 多写者 `SQLITE_BUSY`。
#[derive(Debug, Clone)]
pub struct StatsStore {
    pool: SqlitePool,
}

impl StatsStore {
    /// 打开（不存在则创建）`<data_dir>/stats.db`，执行 schema 初始化与过期清理。
    pub async fn open(data_dir: &Path) -> PanelResult<Self> {
        let path = data_dir.join(STATS_DB_FILE);
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(|e| {
                PanelError::Client(format!("打开统计数据库 {} 失败: {e}", path.display()))
            })?;
        let store = Self { pool };
        store.migrate().await?;
        if let Err(e) = store.cleanup_expired().await {
            tracing::warn!(error = %e, "统计数据过期清理失败（不影响使用）");
        }
        Ok(store)
    }

    /// 初始化 / 升级 schema（`PRAGMA user_version` 版本门控，幂等）。
    async fn migrate(&self) -> PanelResult<()> {
        let version: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| PanelError::Client(format!("读取统计库 schema 版本失败: {e}")))?;
        if version >= SCHEMA_VERSION {
            return Ok(());
        }
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS daily_stats (
                date            TEXT NOT NULL,
                target          TEXT NOT NULL,
                destination_ip  TEXT NOT NULL DEFAULT '',
                rule            TEXT NOT NULL DEFAULT '',
                rule_payload    TEXT NOT NULL DEFAULT '',
                outbound        TEXT NOT NULL DEFAULT '',
                upload_bytes    INTEGER NOT NULL DEFAULT 0,
                download_bytes  INTEGER NOT NULL DEFAULT 0,
                conn_count      INTEGER NOT NULL DEFAULT 0,
                last_seen       INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (date, target, destination_ip, rule, rule_payload, outbound)
            );
            CREATE TABLE IF NOT EXISTS conn_records (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                target          TEXT NOT NULL,
                host            TEXT NOT NULL DEFAULT '',
                destination_ip  TEXT NOT NULL DEFAULT '',
                network         TEXT NOT NULL DEFAULT '',
                chain           TEXT NOT NULL DEFAULT '',
                outbound        TEXT NOT NULL DEFAULT '',
                rule            TEXT NOT NULL DEFAULT '',
                rule_payload    TEXT NOT NULL DEFAULT '',
                upload          INTEGER NOT NULL DEFAULT 0,
                download        INTEGER NOT NULL DEFAULT 0,
                started_at      INTEGER NOT NULL DEFAULT 0,
                ended_at        INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_conn_records_ended ON conn_records(ended_at);",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| PanelError::Client(format!("初始化统计库 schema 失败: {e}")))?;
        sqlx::query(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))
            .execute(&self.pool)
            .await
            .map_err(|e| PanelError::Client(format!("写入统计库 schema 版本失败: {e}")))?;
        Ok(())
    }

    /// 删除过期数据：明细保留 [`RECORDS_RETENTION_DAYS`] 天，聚合保留
    /// [`DAILY_RETENTION_DAYS`] 天（按本地日期）。
    async fn cleanup_expired(&self) -> PanelResult<()> {
        let now = chrono::Local::now();
        let records_cutoff = (now - chrono::Duration::days(RECORDS_RETENTION_DAYS)).timestamp();
        let daily_cutoff = (now - chrono::Duration::days(DAILY_RETENTION_DAYS))
            .format("%Y-%m-%d")
            .to_string();
        sqlx::query("DELETE FROM conn_records WHERE ended_at < ?")
            .bind(records_cutoff)
            .execute(&self.pool)
            .await
            .map_err(|e| PanelError::Client(format!("清理过期连接明细失败: {e}")))?;
        sqlx::query("DELETE FROM daily_stats WHERE date < ?")
            .bind(daily_cutoff)
            .execute(&self.pool)
            .await
            .map_err(|e| PanelError::Client(format!("清理过期日聚合失败: {e}")))?;
        Ok(())
    }

    /// 应用一批快照差分：增量 upsert 日聚合 + 落关闭连接明细（单事务）。
    pub async fn apply_batch(
        &self,
        date: &str,
        deltas: &[StatDelta],
        closed: &[ConnectionView],
    ) -> PanelResult<()> {
        if deltas.is_empty() && closed.is_empty() {
            return Ok(());
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| PanelError::Client(format!("开启统计写入事务失败: {e}")))?;
        for delta in deltas {
            if delta.upload == 0 && delta.download == 0 && !delta.new_conn {
                continue;
            }
            sqlx::query(
                "INSERT INTO daily_stats
                    (date, target, destination_ip, rule, rule_payload, outbound,
                     upload_bytes, download_bytes, conn_count, last_seen)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                 ON CONFLICT(date, target, destination_ip, rule, rule_payload, outbound)
                 DO UPDATE SET
                    upload_bytes   = upload_bytes + excluded.upload_bytes,
                    download_bytes = download_bytes + excluded.download_bytes,
                    conn_count     = conn_count + excluded.conn_count,
                    last_seen      = MAX(last_seen, excluded.last_seen)",
            )
            .bind(date)
            .bind(&delta.target)
            .bind(&delta.destination_ip)
            .bind(&delta.rule)
            .bind(&delta.rule_payload)
            .bind(&delta.outbound)
            .bind(delta.upload as i64)
            .bind(delta.download as i64)
            .bind(i64::from(delta.new_conn))
            .bind(delta.seen_at)
            .execute(&mut *tx)
            .await
            .map_err(|e| PanelError::Client(format!("写入日聚合失败: {e}")))?;
        }
        let now = chrono::Local::now().timestamp();
        for conn in closed {
            sqlx::query(
                "INSERT INTO conn_records
                    (target, host, destination_ip, network, chain, outbound,
                     rule, rule_payload, upload, download, started_at, ended_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&conn.target)
            .bind(&conn.host)
            .bind(&conn.destination_ip)
            .bind(&conn.network)
            .bind(&conn.chain)
            .bind(&conn.outbound)
            .bind(&conn.rule)
            .bind(&conn.rule_payload)
            .bind(conn.upload as i64)
            .bind(conn.download as i64)
            .bind(conn.start as i64)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(|e| PanelError::Client(format!("写入连接明细失败: {e}")))?;
        }
        tx.commit()
            .await
            .map_err(|e| PanelError::Client(format!("提交统计写入事务失败: {e}")))?;
        Ok(())
    }

    /// 今日汇总（卡片数据源）。
    pub async fn today_summary(&self, date: &str) -> PanelResult<TodaySummary> {
        let sql = format!(
            "SELECT
                COALESCE(SUM(upload_bytes), 0)   AS upload_bytes,
                COALESCE(SUM(download_bytes), 0) AS download_bytes,
                COALESCE(SUM(CASE WHEN LOWER(outbound) NOT IN ({DIRECT_TAG_SQL_LIST})
                                  THEN upload_bytes ELSE 0 END), 0)   AS proxied_upload,
                COALESCE(SUM(CASE WHEN LOWER(outbound) NOT IN ({DIRECT_TAG_SQL_LIST})
                                  THEN download_bytes ELSE 0 END), 0) AS proxied_download,
                COALESCE(SUM(conn_count), 0)     AS conn_count,
                COUNT(DISTINCT target)           AS target_count
             FROM daily_stats WHERE date = ?"
        );
        let row = sqlx::query(&sql)
            .bind(date)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| PanelError::Client(format!("查询今日流量汇总失败: {e}")))?;
        Ok(TodaySummary {
            date: date.to_string(),
            upload_bytes: row.get("upload_bytes"),
            download_bytes: row.get("download_bytes"),
            proxied_upload_bytes: row.get("proxied_upload"),
            proxied_download_bytes: row.get("proxied_download"),
            connection_count: row.get("conn_count"),
            target_count: row.get("target_count"),
        })
    }

    /// 日聚合查询（排序 + 模糊搜索）。
    pub async fn query_daily(
        &self,
        query: &DailyQuery,
        default_date: &str,
    ) -> PanelResult<Vec<DailyStatRow>> {
        let date = query
            .date
            .clone()
            .unwrap_or_else(|| default_date.to_string());
        let sort_col = match query.sort {
            DailySort::Upload => "upload_bytes",
            DailySort::Download => "download_bytes",
            DailySort::Total => "(upload_bytes + download_bytes)",
            DailySort::Count => "conn_count",
            DailySort::LastSeen => "last_seen",
        };
        let dir = if query.desc.unwrap_or(true) {
            "DESC"
        } else {
            "ASC"
        };
        let limit = clamp_limit(query.limit);
        let mut sql = String::from(
            "SELECT date, target, destination_ip, rule, rule_payload, outbound,
                    upload_bytes, download_bytes, conn_count, last_seen
             FROM daily_stats WHERE date = ?",
        );
        if query.outbound.as_deref().is_some_and(|o| !o.is_empty()) {
            sql.push_str(" AND outbound = ?");
        }
        if query.search.as_deref().is_some_and(|s| !s.is_empty()) {
            sql.push_str(
                " AND (target LIKE ? ESCAPE '\\' OR destination_ip LIKE ? ESCAPE '\\'
                    OR rule LIKE ? ESCAPE '\\' OR rule_payload LIKE ? ESCAPE '\\'
                    OR outbound LIKE ? ESCAPE '\\')",
            );
        }
        sql.push_str(&format!(
            " ORDER BY {sort_col} {dir}, target ASC LIMIT {limit}"
        ));

        let mut q = sqlx::query_as::<_, DailyStatRow>(&sql).bind(&date);
        if let Some(outbound) = query.outbound.as_deref().filter(|o| !o.is_empty()) {
            q = q.bind(outbound);
        }
        if let Some(search) = query.search.as_deref().filter(|s| !s.is_empty()) {
            // 搜索条件含 5 个匿名占位符（target/destination_ip/rule/rule_payload/outbound），逐个绑定。
            for _ in 0..5 {
                q = q.bind(like_pattern(search));
            }
        }
        let rows = q
            .fetch_all(&self.pool)
            .await
            .map_err(|e| PanelError::Client(format!("查询日聚合失败: {e}")))?;
        Ok(rows)
    }

    /// 关闭连接明细查询（排序 + 模糊搜索）。
    pub async fn query_records(&self, query: &RecordsQuery) -> PanelResult<Vec<ConnRecordRow>> {
        let sort_col = match query.sort {
            RecordSort::Upload => "upload",
            RecordSort::Download => "download",
            RecordSort::Total => "(upload + download)",
            RecordSort::Started => "started_at",
            RecordSort::Ended => "ended_at",
        };
        let dir = if query.desc.unwrap_or(true) {
            "DESC"
        } else {
            "ASC"
        };
        let limit = clamp_limit(query.limit);
        let mut sql = String::from(
            "SELECT id, target, host, destination_ip, network, chain, outbound,
                    rule, rule_payload, upload, download, started_at, ended_at
             FROM conn_records",
        );
        if query.search.as_deref().is_some_and(|s| !s.is_empty()) {
            sql.push_str(
                " WHERE (target LIKE ? ESCAPE '\\' OR host LIKE ? ESCAPE '\\'
                    OR destination_ip LIKE ? ESCAPE '\\' OR rule LIKE ? ESCAPE '\\'
                    OR rule_payload LIKE ? ESCAPE '\\' OR outbound LIKE ? ESCAPE '\\')",
            );
        }
        sql.push_str(&format!(
            " ORDER BY {sort_col} {dir}, id DESC LIMIT {limit}"
        ));

        let mut q = sqlx::query_as::<_, ConnRecordRow>(&sql);
        if let Some(search) = query.search.as_deref().filter(|s| !s.is_empty()) {
            // 搜索条件含 6 个匿名占位符（target/host/destination_ip/rule/rule_payload/outbound），逐个绑定。
            for _ in 0..6 {
                q = q.bind(like_pattern(search));
            }
        }
        let rows = q
            .fetch_all(&self.pool)
            .await
            .map_err(|e| PanelError::Client(format!("查询连接明细失败: {e}")))?;
        Ok(rows)
    }

    /// 清空全部统计数据（聚合 + 明细）。
    pub async fn clear(&self) -> PanelResult<()> {
        sqlx::query("DELETE FROM daily_stats")
            .execute(&self.pool)
            .await
            .map_err(|e| PanelError::Client(format!("清空日聚合失败: {e}")))?;
        sqlx::query("DELETE FROM conn_records")
            .execute(&self.pool)
            .await
            .map_err(|e| PanelError::Client(format!("清空连接明细失败: {e}")))?;
        Ok(())
    }
}

/// 查询条数：默认 [`DEFAULT_QUERY_LIMIT`]，封顶 [`MAX_QUERY_LIMIT`]。
fn clamp_limit(limit: Option<u32>) -> u32 {
    limit
        .unwrap_or(DEFAULT_QUERY_LIMIT)
        .clamp(1, MAX_QUERY_LIMIT)
}

/// 构造 LIKE 模糊匹配模式（转义 `%` / `_` / `\`，两端包裹 `%`）。
fn like_pattern(search: &str) -> String {
    let escaped = search
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}
