use std::sync::Arc;

use rquickjs::function::{Async, Func};
use rquickjs::{Ctx, Function, Object, Value};

use crate::host::ScriptHost;
use crate::types::HttpRequestSpec;

/// 注入 $httpClient.get/post/put/delete/head/options/patch(opts_or_url, callback)。
/// callback(error, response{status,headers}, data)。函数本身 resolve 为 undefined。
pub(crate) fn install<'js>(ctx: &Ctx<'js>, host: &Arc<ScriptHost>) -> rquickjs::Result<()> {
    let http_client = Object::new(ctx.clone())?;

    macro_rules! method {
        ($name:literal, $method:literal) => {{
            let host = Arc::clone(host);
            // Ctx 由框架作为首参注入（不捕获，避免 JS 对象持有 Ctx 形成引用循环）。
            let f = Async(move |ctx: Ctx<'js>, opts_or_url: Value<'js>, cb: Function<'js>| {
                let host = Arc::clone(&host);
                async move {
                    let req = parse_opts(&opts_or_url).map(|mut r| {
                        r.method = $method.to_string();
                        r
                    });
                    match req {
                        Ok(req) => match host.http.execute(req).await {
                            Ok(resp) => {
                                let null_v = Value::new_null(ctx.clone());
                                let resp_val = build_response(&ctx, &resp)
                                    .unwrap_or_else(|_| Value::new_null(ctx.clone()));
                                let body = resp.body;
                                let _ = cb.call::<(Value<'js>, Value<'js>, String), ()>(
                                    (null_v, resp_val, body),
                                );
                                Ok::<(), rquickjs::Error>(())
                            }
                            Err(e) => {
                                let null_v = Value::new_null(ctx.clone());
                                let msg = e.to_string();
                                let _ = cb.call::<(String, Value<'js>, Value<'js>), ()>(
                                    (msg, null_v.clone(), null_v),
                                );
                                Ok::<(), rquickjs::Error>(())
                            }
                        },
                        Err(e) => {
                            let null_v = Value::new_null(ctx.clone());
                            let msg = e.to_string();
                            let _ = cb.call::<(String, Value<'js>, Value<'js>), ()>((
                                msg,
                                null_v.clone(),
                                null_v,
                            ));
                            Ok::<(), rquickjs::Error>(())
                        }
                    }
                }
            });
            let func = Func::new(f);
            http_client.set($name, func)?;
        }};
    }

    method!("get", "GET");
    method!("post", "POST");
    method!("put", "PUT");
    method!("delete", "DELETE");
    method!("head", "HEAD");
    method!("options", "OPTIONS");
    method!("patch", "PATCH");

    ctx.globals().set("$httpClient", http_client)?;
    Ok(())
}

/// 构造 response 对象 {status, headers}。
fn build_response<'js>(
    ctx: &Ctx<'js>,
    resp: &crate::types::HttpResponseData,
) -> rquickjs::Result<Value<'js>> {
    let obj = Object::new(ctx.clone())?;
    obj.set("status", resp.status)?;
    let headers = Object::new(ctx.clone())?;
    for (k, v) in &resp.headers {
        headers.set(k.as_str(), v.as_str())?;
    }
    obj.set("headers", headers)?;
    Ok(obj.into_value())
}

/// 解析 $httpClient 的参数：url 字符串或对象 {url,method,headers,body,timeout}。
fn parse_opts<'js>(opts: &Value<'js>) -> rquickjs::Result<HttpRequestSpec> {
    if let Some(s) = opts.as_string() {
        return Ok(HttpRequestSpec {
            url: s.to_string()?,
            method: "GET".to_string(),
            ..HttpRequestSpec::default()
        });
    }
    let Some(obj) = opts.as_object() else {
        return Err(rquickjs::Error::new_from_js("opts", "HttpRequestSpec"));
    };
    let url: String = obj.get("url")?;
    if url.is_empty() {
        return Err(rquickjs::Error::new_from_js("opts", "HttpRequestSpec"));
    }
    let method: Option<String> = obj.get("method")?;
    let body: Option<String> = obj.get("body")?;
    let timeout: Option<u64> = obj.get("timeout")?;
    let mut headers = Vec::new();
    if let Some(h) = obj.get::<_, Option<Object>>("headers")? {
        for entry in h.props::<rquickjs::Atom, Value>() {
            let (k, v) = entry?;
            if let Some(v) = v.as_string() {
                headers.push((k.to_string()?, v.to_string()?));
            }
        }
    }
    Ok(HttpRequestSpec {
        url,
        method: method.unwrap_or_else(|| "GET".to_string()),
        headers,
        body,
        timeout_ms: timeout,
    })
}
