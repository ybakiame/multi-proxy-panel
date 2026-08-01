use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rquickjs::function::Func;
use rquickjs::{Ctx, Function, Object, Value};
use tokio::sync::oneshot;

use crate::api::value_to_string;
use crate::host::{ScriptHost, TimerRegistry};
use crate::types::{ScriptKind, ScriptOutput};

/// 注入公共 API：$done / console / setTimeout / $environment / $script。
pub(crate) fn install<'js>(
    ctx: &Ctx<'js>,
    _host: &Arc<ScriptHost>,
    done_tx: oneshot::Sender<ScriptOutput>,
    script_name: &str,
    kind: ScriptKind,
    timers: &TimerRegistry,
) -> rquickjs::Result<()> {
    let globals = ctx.globals();

    // $done(value)：向 Rust 侧 oneshot 回传结果。多次调用时后续忽略（send 失败）。
    // Sender::send 消耗自身，故用 Mutex<Option<Sender>> 包裹，保证闭包是 Fn。
    // Ctx 由框架作为首参注入（不捕获，避免 JS 对象持有 Ctx 形成引用循环）。
    let done_tx = std::sync::Mutex::new(Some(done_tx));
    let done_fn = move |ctx: Ctx<'js>, value: Value<'js>| -> rquickjs::Result<()> {
        // QX 语义：$done 可接收对象或 JSON 字符串；字符串先尝试解析为 JSON 对象。
        let mut json = crate::api::js_to_json(&ctx, value)?;
        if let serde_json::Value::String(s) = &json {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(s) {
                json = parsed;
            }
        }
        if let Ok(mut tx) = done_tx.lock() {
            if let Some(tx) = tx.take() {
                let _ = tx.send(ScriptOutput(json));
            }
        }
        Ok(())
    };
    globals.set("$done", Func::from(done_fn))?;

    // console.log(...args)
    let console = Object::new(ctx.clone())?;
    let log_fn =
        move |ctx: Ctx<'js>, args: rquickjs::prelude::Rest<Value<'js>>| -> rquickjs::Result<()> {
            let parts: Vec<String> = args
                .0
                .iter()
                .map(|v| value_to_string(&ctx, v.clone()))
                .collect();
            tracing::info!(target: "pp_script::console", "{}", parts.join(" "));
            Ok(())
        };
    console.set("log", Func::from(log_fn))?;
    globals.set("console", console)?;

    // setTimeout(callback, delay_ms)：把回调 Persistent 化存入注册表，返回 undefined。
    // 不 spawn future、不持有 'js 引用；由 run_script 在等待 $done 期间驱动到期回调，
    // 并在 runtime 释放前清空注册表。
    let registry = timers.clone();
    let timeout_fn = move |ctx: Ctx<'js>, cb: Function<'js>, ms: i64| -> rquickjs::Result<()> {
        let due = Instant::now() + Duration::from_millis(ms.max(0) as u64);
        let persistent = rquickjs::Persistent::save(&ctx, cb);
        if let Ok(mut timers) = registry.try_borrow_mut() {
            timers.push((due, persistent));
        }
        Ok(())
    };
    globals.set("setTimeout", Func::from(timeout_fn))?;

    // $environment
    let environment = Object::new(ctx.clone())?;
    environment.set("system", std::env::consts::OS)?;
    environment.set("engine", "quickjs")?;
    globals.set("$environment", environment)?;

    // $script
    let script = Object::new(ctx.clone())?;
    script.set("name", script_name)?;
    script.set("startTime", format_start_time())?;
    script.set("type", kind_name(kind))?;
    globals.set("$script", script)?;

    Ok(())
}

fn format_start_time() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    format!("{secs}")
}

fn kind_name(kind: ScriptKind) -> &'static str {
    match kind {
        ScriptKind::HttpRequest => "http-request",
        ScriptKind::HttpResponse => "http-response",
        ScriptKind::Cron => "cron",
        ScriptKind::Generic => "generic",
    }
}
