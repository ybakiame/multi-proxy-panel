use std::sync::Arc;

use rquickjs::function::{Func, Opt};
use rquickjs::{Ctx, Object, Value};

use crate::host::ScriptHost;

/// 注入 $notify（QX）：$notify(title, subtitle, body[, opts])。
pub(crate) fn install_qx<'js>(ctx: &Ctx<'js>, host: &Arc<ScriptHost>) -> rquickjs::Result<()> {
    let h = Arc::clone(host);
    // Ctx 由框架作为首参注入（不捕获，避免 JS 对象持有 Ctx 形成引用循环）。
    // opts 用 Opt<Value>（rquickjs 可选参数），否则 Option 会被计为必传参数。
    let f = move |ctx: Ctx<'js>,
                  title: String,
                  subtitle: String,
                  body: String,
                  opts: Opt<Value<'js>>|
          -> rquickjs::Result<()> {
        let options = convert_opts(&ctx, opts)?;
        h.notifier.notify(&title, &subtitle, &body, options);
        Ok(())
    };
    ctx.globals().set("$notify", Func::from(f))?;
    Ok(())
}

/// 注入 $notification（Surge）：$notification.post(title, subtitle, body[, opts])。
pub(crate) fn install_surge<'js>(ctx: &Ctx<'js>, host: &Arc<ScriptHost>) -> rquickjs::Result<()> {
    let h = Arc::clone(host);
    let f = move |ctx: Ctx<'js>,
                  title: String,
                  subtitle: String,
                  body: String,
                  opts: Opt<Value<'js>>|
          -> rquickjs::Result<()> {
        let options = convert_opts(&ctx, opts)?;
        h.notifier.notify(&title, &subtitle, &body, options);
        Ok(())
    };
    let notification = Object::new(ctx.clone())?;
    notification.set("post", Func::from(f))?;
    ctx.globals().set("$notification", notification)?;
    Ok(())
}

/// 将可选 JS opts 值转为 serde_json::Value。
fn convert_opts<'js>(
    ctx: &Ctx<'js>,
    opts: Opt<Value<'js>>,
) -> rquickjs::Result<Option<serde_json::Value>> {
    match opts.0 {
        Some(v) => Ok(Some(crate::api::js_to_json(ctx, v)?)),
        None => Ok(None),
    }
}
