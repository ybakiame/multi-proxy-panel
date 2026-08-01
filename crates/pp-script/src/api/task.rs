use std::sync::Arc;

use rquickjs::function::{Async, Func};
use rquickjs::{Ctx, Object, Value};

use crate::host::ScriptHost;
use crate::types::{HttpRequestSpec, HttpResponseData};

/// 注入 $task.fetch(opts) -> Promise。opts 为 url 字符串或对象 {url,method,headers,body,timeout}。
pub(crate) fn install<'js>(ctx: &Ctx<'js>, host: &Arc<ScriptHost>) -> rquickjs::Result<()> {
    let task = Object::new(ctx.clone())?;
    let host = Arc::clone(host);

    // Ctx 由框架作为首参注入（不捕获，避免 JS 对象持有 Ctx 形成引用循环）。
    let fetch_fn = Async(move |ctx: Ctx<'js>, opts: Value<'js>| {
        let host = Arc::clone(&host);
        async move {
            let req = parse_opts(&opts);
            // resolve {statusCode, headers, body} / reject error string
            match req {
                Ok(req) => match host.http.execute(req).await {
                    Ok(resp) => Ok(TaskFetchResult::from(resp)),
                    Err(e) => Err(throw_error(&ctx, &e.to_string())),
                },
                Err(e) => Err(throw_error(&ctx, &e.to_string())),
            }
        }
    });
    task.set("fetch", Func::from(fetch_fn))?;
    ctx.globals().set("$task", task)?;
    Ok(())
}

/// 构造一个 JS 异常（Error 对象）并返回 `Error::Exception`。
fn throw_error<'js>(ctx: &Ctx<'js>, message: &str) -> rquickjs::Error {
    let err = rquickjs::String::from_str(ctx.clone(), message)
        .ok()
        .map(|s| s.into_value());
    match err {
        Some(v) => ctx.throw(v),
        None => rquickjs::Error::new_from_js("script", "value"),
    }
}

/// $task.fetch 的 resolve 值：{statusCode, headers(对象), body}。
struct TaskFetchResult {
    status_code: u16,
    headers: Vec<(String, String)>,
    body: String,
}

impl From<HttpResponseData> for TaskFetchResult {
    fn from(r: HttpResponseData) -> Self {
        Self {
            status_code: r.status,
            headers: r.headers,
            body: r.body,
        }
    }
}

impl<'js> rquickjs::IntoJs<'js> for TaskFetchResult {
    fn into_js(self, ctx: &Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        let obj = Object::new(ctx.clone())?;
        obj.set("statusCode", self.status_code)?;
        let headers = Object::new(ctx.clone())?;
        for (k, v) in &self.headers {
            headers.set(k.as_str(), v.as_str())?;
        }
        obj.set("headers", headers)?;
        obj.set("body", self.body)?;
        Ok(obj.into_value())
    }
}

/// 解析 $task.fetch 的参数：url 字符串或对象。
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
