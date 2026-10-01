//! Traffic statistics commands: today summary, daily aggregates, closed-connection records.

use pp_client::{ConnRecordRow, DailyQuery, DailyStatRow, RecordsQuery, TodaySummary};
use tauri::State;

use crate::state::AppState;

/// 获取今日流量汇总（首页卡片数据源；核心未运行也可查询历史数据）。
#[tauri::command]
pub async fn stats_today(state: State<'_, AppState>) -> Result<TodaySummary, String> {
    let store = state
        .stats_store()
        .await
        .ok_or_else(|| "统计存储不可用".to_string())?;
    let date = pp_client::today();
    store.today_summary(&date).await.map_err(|e| e.to_string())
}

/// 查询日聚合（按 域名/IP + 规则 + 出站 维度，支持排序与模糊搜索）。
#[tauri::command]
pub async fn stats_daily(
    state: State<'_, AppState>,
    query: DailyQuery,
) -> Result<Vec<DailyStatRow>, String> {
    let store = state
        .stats_store()
        .await
        .ok_or_else(|| "统计存储不可用".to_string())?;
    let default_date = pp_client::today();
    store
        .query_daily(&query, &default_date)
        .await
        .map_err(|e| e.to_string())
}

/// 查询关闭连接明细（支持排序与模糊搜索）。
#[tauri::command]
pub async fn stats_records(
    state: State<'_, AppState>,
    query: RecordsQuery,
) -> Result<Vec<ConnRecordRow>, String> {
    let store = state
        .stats_store()
        .await
        .ok_or_else(|| "统计存储不可用".to_string())?;
    store.query_records(&query).await.map_err(|e| e.to_string())
}

/// 清空全部统计数据（聚合 + 明细）。
#[tauri::command]
pub async fn stats_clear(state: State<'_, AppState>) -> Result<(), String> {
    let store = state
        .stats_store()
        .await
        .ok_or_else(|| "统计存储不可用".to_string())?;
    store.clear().await.map_err(|e| e.to_string())
}
