use std::sync::Arc;

use rquickjs::function::{Func, Opt};
use rquickjs::{Ctx, Object};

use crate::host::ScriptHost;

/// QX 与 Surge 持久化 API 的 scope 前缀。
const QX_SCOPE_PREFIX: &str = "prefs";
const SURGE_SCOPE_PREFIX: &str = "pstore";

/// 无 key 调用时的固定单值槽（模拟 QX 脚本级共享池）。
const DEFAULT_KEY: &str = "__default__";

/// 注入 $prefs（QX）：valueForKey / setValueForKey / removeValueForKey。
pub(crate) fn install_qx<'js>(
    ctx: &Ctx<'js>,
    host: &Arc<ScriptHost>,
    script_name: &str,
) -> rquickjs::Result<()> {
    let prefs = Object::new(ctx.clone())?;
    let base_scope = format!("{QX_SCOPE_PREFIX}:{script_name}");

    let s = Arc::clone(host);
    let scope = base_scope.clone();
    prefs.set(
        "valueForKey",
        Func::from(move |key: Opt<String>| -> rquickjs::Result<Option<String>> {
            let k = key.0.unwrap_or_else(|| DEFAULT_KEY.to_string());
            Ok(s.store.read(&scope, &k))
        }),
    )?;

    let s = Arc::clone(host);
    let scope = base_scope.clone();
    prefs.set(
        "setValueForKey",
        Func::from(move |value: String, key: Opt<String>| -> rquickjs::Result<()> {
            let k = key.0.unwrap_or_else(|| DEFAULT_KEY.to_string());
            s.store.write(&scope, &k, &value);
            Ok(())
        }),
    )?;

    let s = Arc::clone(host);
    let scope = base_scope.clone();
    prefs.set(
        "removeValueForKey",
        Func::from(move |key: Opt<String>| -> rquickjs::Result<()> {
            let k = key.0.unwrap_or_else(|| DEFAULT_KEY.to_string());
            s.store.erase(&scope, &k);
            Ok(())
        }),
    )?;

    ctx.globals().set("$prefs", prefs)?;
    Ok(())
}

/// 注入 $persistentStore（Surge）：read / write / erase。
pub(crate) fn install_surge<'js>(
    ctx: &Ctx<'js>,
    host: &Arc<ScriptHost>,
    script_name: &str,
) -> rquickjs::Result<()> {
    let pstore = Object::new(ctx.clone())?;
    let base_scope = format!("{SURGE_SCOPE_PREFIX}:{script_name}");

    let s = Arc::clone(host);
    let scope = base_scope.clone();
    pstore.set(
        "read",
        Func::from(move |key: String| -> rquickjs::Result<Option<String>> {
            Ok(s.store.read(&scope, &key))
        }),
    )?;

    let s = Arc::clone(host);
    let scope = base_scope.clone();
    pstore.set(
        "write",
        // Surge 语义：$persistentStore.write(data, key) —— 数据在前、key 在后。
        Func::from(move |data: String, key: String| -> rquickjs::Result<()> {
            s.store.write(&scope, &key, &data);
            Ok(())
        }),
    )?;

    let s = Arc::clone(host);
    let scope = base_scope.clone();
    pstore.set(
        "erase",
        Func::from(move |key: String| -> rquickjs::Result<()> {
            s.store.erase(&scope, &key);
            Ok(())
        }),
    )?;

    ctx.globals().set("$persistentStore", pstore)?;
    Ok(())
}
