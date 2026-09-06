/**
 * Task script (cron scheduling) API types and functions.
 *
 * Aligned with Rust-side `TaskScriptView`.
 */

import { invoke } from "@tauri-apps/api/core";

/** Task script view (aligned with `TaskScriptView`, fields are snake_case raw serialization). */
export interface TaskScriptView {
  name: string;
  cron_expr: string;
  dialect: string;
  enabled: boolean;
  next_run: string | null;
  last_run: string | null;
  last_error: string | null;
}

export function listTasks(): Promise<TaskScriptView[]> {
  return invoke<TaskScriptView[]>("list_tasks");
}

export function runTask(name: string): Promise<string> {
  return invoke<string>("run_task", { name });
}
