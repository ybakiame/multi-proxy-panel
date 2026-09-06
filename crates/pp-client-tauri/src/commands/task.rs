//! Task / cron commands.

use pp_script::TaskScriptView;
use tauri::State;

use crate::state::AppState;

/// List scheduled tasks; returns empty list when client not started or scheduler not ready.
#[tauri::command]
pub async fn list_tasks(state: State<'_, AppState>) -> Result<Vec<TaskScriptView>, String> {
    let lock = state.client.lock().await;
    let Some(client) = lock.as_ref() else {
        return Ok(Vec::new());
    };
    let Some(scheduler) = client.scheduler() else {
        return Ok(Vec::new());
    };
    Ok(scheduler.list_tasks())
}

/// Manually run a scheduled task; returns the script `$done` output JSON string.
#[tauri::command]
pub async fn run_task(state: State<'_, AppState>, name: String) -> Result<String, String> {
    let scheduler = {
        let lock = state.client.lock().await;
        let client = lock
            .as_ref()
            .ok_or_else(|| "客户端未启动，无法运行任务".to_string())?;
        client
            .scheduler_handle()
            .ok_or_else(|| "任务调度器未就绪".to_string())?
    };
    let output = scheduler
        .run_now(&name)
        .await
        .map_err(|e| format!("运行任务失败: {e}"))?;
    Ok(output.0.to_string())
}
