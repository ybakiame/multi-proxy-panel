use std::sync::Arc;
use std::time::{Duration, Instant};

use rquickjs::{AsyncContext, AsyncRuntime};
use tokio::sync::oneshot;

use crate::api;
use crate::engine::ScriptEngine;
use crate::host::ScriptHost;
use crate::types::{ScriptDialect, ScriptKind, ScriptLimits, ScriptOutput};
use pp_common::{PanelError, PanelResult};

/// QuickJS（rquickjs）脚本引擎实现。
pub struct QuickJsEngine {
    host: Arc<ScriptHost>,
    dialect: ScriptDialect,
    limits: ScriptLimits,
    script_name: String,
}

impl QuickJsEngine {
    /// 构造签名约定（与未来 `engine-jsc` 后端保持一致的形状）。
    #[allow(clippy::result_large_err)]
    pub fn new(
        host: Arc<ScriptHost>,
        dialect: ScriptDialect,
        limits: ScriptLimits,
        script_name: String,
    ) -> PanelResult<Self> {
        Ok(Self {
            host,
            dialect,
            limits,
            script_name,
        })
    }
}

impl ScriptEngine for QuickJsEngine {
    #[allow(clippy::let_and_return)]
    async fn run_script(
        &mut self,
        source: &str,
        kind: ScriptKind,
        arg: Option<serde_json::Value>,
    ) -> PanelResult<ScriptOutput> {
        let timeout_ms = self.limits.timeout_ms;
        let timeout = Duration::from_millis(timeout_ms);

        let rt = AsyncRuntime::new().map_err(|e| PanelError::Script(format!("init runtime: {e}")))?;
        rt.set_memory_limit(self.limits.memory_limit_bytes).await;
        // 默认栈限制：4MB（足够常见脚本，防深度递归撑爆宿主栈）。
        rt.set_max_stack_size(4 * 1024 * 1024).await;

        // interrupt handler：超过 deadline 即中断（返回 true 抛出不可捕获异常）。
        let deadline = Instant::now() + timeout;
        rt.set_interrupt_handler(Some(Box::new(move || Instant::now() > deadline)))
            .await;

        let ctx = AsyncContext::full(&rt)
            .await
            .map_err(|e| PanelError::Script(format!("init context: {e}")))?;

        let (done_tx, mut done_rx) = oneshot::channel::<ScriptOutput>();
        let host = Arc::clone(&self.host);
        let dialect = self.dialect;
        let script_name = self.script_name.clone();

        // setTimeout 注册表：闭包内注入时写入，闭包返回后由本层在锁内清空。
        let timers = crate::host::TimerRegistry::default();
        let timers_inner = timers.clone();

        let result = ctx
            .async_with(async move |js_ctx| {
                // 注入全部全局 API（$done 的 sender 移入其中）。
                api::install_apis(&js_ctx, &host, dialect, kind, done_tx, &script_name, &timers_inner)
                    .map_err(|e| PanelError::Script(format!("install api: {e}")))?;
                // 按脚本类型注入 $request / $response。
                api::inject_script_arg(&js_ctx, kind, arg.as_ref())
                    .map_err(|e| PanelError::Script(format!("inject arg: {e}")))?;

                // eval 脚本：可能同步调用 $done，也可能在 Promise resolve 后调用。
                let eval_val = match js_ctx.eval::<rquickjs::Value, _>(source) {
                    Ok(v) => v,
                    Err(e) => {
                        let msg = if js_ctx.has_exception() {
                            let exc = js_ctx.catch();
                            api::value_to_string(&js_ctx, exc)
                        } else {
                            format!("{e}")
                        };
                        return Err(PanelError::Script(msg));
                    }
                };
                // 顶层 async IIFE 会返回 Promise；驱动它完成（$task.fetch 等异步 API 依赖此）。
                // 非 Promise 值立即 ready。
                let top = rquickjs::promise::MaybePromise::from_value(eval_val)
                    .into_future::<rquickjs::Value>();
                let mut top_fut = Some(Box::pin(top));

                // 等待 $done，期间驱动 setTimeout 注册表中的到期回调。
                loop {
                    // 0. 整体超时检查。
                    if Instant::now() >= deadline {
                        break Err(PanelError::Script(format!(
                            "script timeout: $done not called within {timeout_ms}ms"
                        )));
                    }

                    // 1. 执行所有已到期 timer（Persistent restore 后调用）。
                    let due: Vec<rquickjs::Persistent<rquickjs::Function<'static>>> = {
                        let mut guard = timers_inner.borrow_mut();
                        let now = Instant::now();
                        let mut kept = Vec::new();
                        let mut fired = Vec::new();
                        for (due_at, cb) in guard.drain(..) {
                            if due_at <= now {
                                fired.push(cb);
                            } else {
                                kept.push((due_at, cb));
                            }
                        }
                        *guard = kept;
                        fired
                    };
                    for cb in due {
                        // async_with 闭包内 js_ctx 已持有 runtime 锁，可直接调用。
                        let restored = cb.restore(&js_ctx);
                        if let Ok(f) = restored {
                            let _ = f.call::<_, ()>(());
                        }
                    }

                    // 2. 计算下一个到期 timer 的等待时长（不超过剩余 deadline）。
                    let next_due = timers_inner.borrow().iter().map(|(t, _)| *t).min();
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    let wait = match next_due {
                        Some(t) => t.saturating_duration_since(Instant::now()).min(remaining),
                        None => remaining,
                    };

                    // 3. 等待 $done / 顶层 Promise 完成 / 下一个 timer 到期。
                    tokio::select! {
                        res = &mut done_rx => {
                            match res {
                                Ok(output) => break Ok(output),
                                Err(_) => {
                                    // $done 未被调用但脚本已结束（sender 被 drop）。
                                    tracing::warn!(script = %script_name, "script finished without calling $done");
                                    break Ok(ScriptOutput::default());
                                }
                            }
                        }
                        top_result = async {
                            let fut = top_fut.as_mut().expect("guarded by is_some");
                            fut.await
                        }, if top_fut.is_some() => {
                            // 顶层 Promise 完成：$done 可能已在其 resolve 链中调用。
                            top_fut = None;
                            // 顶层 Promise 被 reject：提取 JS 异常并终止脚本，避免静默空转到超时。
                            if let Err(e) = top_result {
                                let msg = if js_ctx.has_exception() {
                                    api::value_to_string(&js_ctx, js_ctx.catch())
                                } else {
                                    format!("{e}")
                                };
                                break Err(PanelError::Script(format!(
                                    "top-level promise rejected: {msg}"
                                )));
                            }
                        }
                        _ = tokio::time::sleep(wait) => {
                            // timer 到期或等待超时：由循环体处理到期 timer 与 deadline 检查。
                        }
                    }
                }
            })
            .await;

        // 所有返回路径：在锁内清空 timer 注册表（drop 全部 Persistent），
        // 再强制 GC 与 idle，避免 runtime 释放时 gc_obj_list 断言失败。
        ctx.with(|js_ctx| {
            timers.borrow_mut().clear();
            js_ctx.run_gc();
        })
        .await;
        rt.idle().await;

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host::{MemoryPersistentStore, MockHttpExecutor, PersistentStore, RecordingNotifier};
    use crate::types::{HttpResponseData, ScriptDialect, ScriptLimits};

    fn test_host() -> (Arc<ScriptHost>, Arc<MockHttpExecutor>, Arc<RecordingNotifier>, Arc<MemoryPersistentStore>) {
        let http = Arc::new(MockHttpExecutor::with_responses(vec![
            HttpResponseData {
                status: 200,
                headers: vec![("content-type".into(), "application/json".into())],
                body: r#"{"code":0,"token":"tok123"}"#.into(),
            },
            HttpResponseData {
                status: 200,
                headers: vec![],
                body: r#"{"code":0}"#.into(),
            },
        ]));
        let store = Arc::new(MemoryPersistentStore::new());
        let notifier = Arc::new(RecordingNotifier::new());
        let host = Arc::new(ScriptHost::new(
            http.clone(),
            store.clone(),
            notifier.clone(),
        ));
        (host, http, notifier, store)
    }

    #[tokio::test(flavor = "current_thread")]
    async fn qx_signin_style() {
        let (host, _http, notifier, store) = test_host();
        let mut engine = QuickJsEngine::new(
            host,
            ScriptDialect::QuantumultX,
            ScriptLimits::default(),
            "qx_signin".to_string(),
        )
        .unwrap();

        let source = r#"
            (async () => {
                const resp = await $task.fetch({url: "https://example.com/api/signin", method: "POST", body: "u=1"});
                const data = JSON.parse(resp.body);
                $prefs.setValueForKey(data.token, "token");
                $notify("签到成功", "token=" + data.token, resp.body);
                $done(JSON.stringify({code: data.code, token: data.token}));
            })();
        "#;
        let out = engine.run_script(source, ScriptKind::Generic, None).await.unwrap();
        let v = out.0;
        assert_eq!(v["code"], 0);
        assert_eq!(v["token"], "tok123");
        // store 有值（scope 为 prefs:qx_signin）
        assert_eq!(
            store.read("prefs:qx_signin", "token"),
            Some("tok123".to_string())
        );
        // notifier 记录 1 条
        let calls = notifier.calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].title, "签到成功");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn surge_http_response_rewrite() {
        let (host, _http, _notifier, _store) = test_host();
        let mut engine = QuickJsEngine::new(
            host,
            ScriptDialect::Surge,
            ScriptLimits::default(),
            "rewrite".to_string(),
        )
        .unwrap();

        let arg = serde_json::json!({
            "status": 200,
            "headers": {"content-type": "application/json"},
            "body": r#"{"a":1,"b":2}"#,
        });
        let source = r#"
            const data = JSON.parse($response.body);
            data.a = 99;
            data.newField = "added";
            $done({body: JSON.stringify(data)});
        "#;
        let out = engine
            .run_script(source, ScriptKind::HttpResponse, Some(arg))
            .await
            .unwrap();
        let body = out.0["body"].as_str().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(body).unwrap();
        assert_eq!(parsed["a"], 99);
        assert_eq!(parsed["newField"], "added");
        assert_eq!(parsed["b"], 2);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn loon_superset() {
        let (host, _http, _notifier, store) = test_host();
        let mut engine = QuickJsEngine::new(
            host,
            ScriptDialect::Loon,
            ScriptLimits::default(),
            "loon_test".to_string(),
        )
        .unwrap();

        let source = r#"
            $persistentStore.write("pv1", "k1");
            $httpClient.get({url: "https://example.com/", headers: {"x-a": "1"}}, (error, response, data) => {
                if (error) {
                    $done({err: error});
                } else {
                    $done({status: response.status, body: data});
                }
            });
        "#;
        let out = engine.run_script(source, ScriptKind::Generic, None).await.unwrap();
        assert_eq!(out.0["status"], 200);
        // Loon 的 $persistentStore 与 $httpClient 均生效
        assert_eq!(store.read("pstore:loon_test", "k1"), Some("pv1".to_string()));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn infinite_loop_timeout() {
        let (host, _http, _notifier, _store) = test_host();
        let mut engine = QuickJsEngine::new(
            host,
            ScriptDialect::QuantumultX,
            ScriptLimits {
                timeout_ms: 500,
                ..ScriptLimits::default()
            },
            "loop".to_string(),
        )
        .unwrap();

        let source = "while(true) {}";
        let err = engine.run_script(source, ScriptKind::Generic, None).await.unwrap_err();
        assert!(matches!(err, PanelError::Script(_)), "expected Script error, got {err:?}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn thrown_error_captured() {
        let (host, _http, _notifier, _store) = test_host();
        let mut engine = QuickJsEngine::new(
            host,
            ScriptDialect::QuantumultX,
            ScriptLimits::default(),
            "throw_test".to_string(),
        )
        .unwrap();

        let source = r#"throw new Error("boom");"#;
        let err = engine.run_script(source, ScriptKind::Generic, None).await.unwrap_err();
        let PanelError::Script(msg) = &err else {
            panic!("expected Script error, got {err:?}");
        };
        assert!(msg.contains("boom"), "error message should contain boom, got: {msg}");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mini_promise_drive_experiment() {
        let (host, _h, _n, _s) = test_host();
        let mut engine = QuickJsEngine::new(
            host,
            ScriptDialect::QuantumultX,
            ScriptLimits::default(),
            "mini".to_string(),
        )
        .unwrap();
        // 简单同步脚本，不产生顶层 Promise
        let out = engine
            .run_script("$done({x: 1});", ScriptKind::Generic, None)
            .await;
        assert!(out.is_ok(), "err: {:?}", out.err());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mini_bare_rquickjs() {
        // 完全不经过我们的 API 注入，直接裸 rquickjs。
        let rt = rquickjs::AsyncRuntime::new().unwrap();
        let ctx = rquickjs::AsyncContext::full(&rt).await.unwrap();
        let res: i32 = ctx.async_with(async |c| c.eval("1 + 1").unwrap()).await;
        assert_eq!(res, 2);
        // 简单 eval 后 drop runtime，观察是否断言失败
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mini_bare_with_globals() {
        // 裸 rquickjs + 注入一个普通函数对象，观察是否泄漏
        let rt = rquickjs::AsyncRuntime::new().unwrap();
        let ctx = rquickjs::AsyncContext::full(&rt).await.unwrap();
        ctx.async_with(async |c| {
            let globals = c.globals();
            let f = |a: i64| -> rquickjs::Result<i64> { Ok(a + 1) };
            globals.set("f", rquickjs::function::Func::from(f)).unwrap();
            let res: i64 = c.eval("f(41)").unwrap();
            assert_eq!(res, 42);
        })
        .await;
    }



}
