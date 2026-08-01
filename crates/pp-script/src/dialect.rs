use crate::types::ScriptDialect;

/// 方言处理：同一 host 能力按方言注入不同全局名。
///
/// 各方言的全局 API 集合：
/// - QuantumultX：$task / $prefs / $notify / $done
/// - Surge：$httpClient / $persistentStore / $notification / $done
/// - Loon：以上全部 + $loon 标记对象（Loon 是超集）
pub fn dialect_globals(dialect: ScriptDialect) -> &'static [&'static str] {
    match dialect {
        ScriptDialect::QuantumultX => &["$task", "$prefs", "$notify"],
        ScriptDialect::Surge => &["$httpClient", "$persistentStore", "$notification"],
        ScriptDialect::Loon => &[
            "$task",
            "$prefs",
            "$notify",
            "$httpClient",
            "$persistentStore",
            "$notification",
            "$loon",
        ],
    }
}
