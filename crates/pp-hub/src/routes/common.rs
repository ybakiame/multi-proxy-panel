use std::collections::HashMap;

/// Parse optional `page` and `per_page` query parameters.
/// Returns `Some((page, per_page))` when both are present, otherwise `None`
/// indicating the caller should return all records (backward compatibility).
pub fn parse_pagination(params: &HashMap<String, String>) -> Option<(u64, u64)> {
    let page = params.get("page").and_then(|s| s.parse::<u64>().ok())?;
    let per_page = params.get("per_page").and_then(|s| s.parse::<u64>().ok())?;
    Some((page.max(1), per_page.clamp(1, 1000)))
}
