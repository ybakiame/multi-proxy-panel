//! Version-related free functions for core management (sing-box only).

use std::path::Path;
use std::process::Command;

use pp_common::{CoreType, PanelError, PanelResult};

/// Core directory / binary base name.
pub(super) fn binary_name() -> &'static str {
    "sing-box"
}

/// On-disk binary file name (append `.exe` on Windows).
pub(super) fn binary_name_on_disk() -> String {
    let base = binary_name();
    if std::env::consts::OS == "windows" {
        format!("{base}.exe")
    } else {
        base.to_string()
    }
}

/// Infer core type from file name: file name (case-insensitive) contains `sing-box` / `singbox` →
/// [`CoreType::SingBox`]; returns `None` when unrecognized (command layer prompts user to
/// manually select).
///
/// Used by command layer (`set_active_core` fallback when path not in inventory) and tests.
#[allow(dead_code)]
pub fn infer_core_type(path: &Path) -> Option<CoreType> {
    let name = path.file_name()?.to_string_lossy().to_lowercase();
    if name.contains("sing-box") || name.contains("singbox") {
        Some(CoreType::SingBox)
    } else {
        None
    }
}

/// Current platform asset hint: `("os-arch", is_windows)`.
pub(super) fn target_spec() -> PanelResult<(&'static str, bool)> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok(("linux-amd64", false)),
        ("linux", "aarch64") => Ok(("linux-arm64", false)),
        ("macos", "x86_64") => Ok(("darwin-amd64", false)),
        ("macos", "aarch64") => Ok(("darwin-arm64", false)),
        ("windows", "x86_64") => Ok(("windows-amd64", true)),
        ("windows", "aarch64") => Ok(("windows-arm64", true)),
        (os, arch) => Err(PanelError::Core(format!(
            "Unsupported core download platform: {os}-{arch}"
        ))),
    }
}

/// Normalize version to GitHub tag (stable versions get `v` prefix).
pub(super) fn github_tag(version: &str) -> String {
    if version.starts_with('v') {
        version.to_string()
    } else {
        format!("v{version}")
    }
}

/// Whether asset extension matches current platform.
pub(super) fn ext_ok(is_windows: bool, name: &str) -> bool {
    if is_windows {
        name.ends_with(".zip")
    } else {
        name.ends_with(".tar.gz")
    }
}

/// Try `version` subcommand / `--version` / `-v` in sequence, take first output with exit code 0
/// (concatenate stdout / stderr).
///
/// sing-box 1.14+ removed `--version` flag, using `version` subcommand instead. Unified probing
/// order: `version` → `--version` → `-v`, compatible with both old and new cores.
pub(super) fn binary_output(binary: &Path) -> String {
    for arg in ["version", "--version", "-v"] {
        if let Ok(output) = Command::new(binary).arg(arg).output() {
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            if output.status.success() {
                return text;
            }
        }
    }
    String::new()
}

/// Parse version number from version probe output.
///
/// sing-box: `sing-box version 1.13.15`; `version` subcommand output may contain multiple lines
/// (first line `sing-box version 1.14.0-beta.4` + environment info), take first line containing
/// "version".
pub(super) fn parse_version_from_output(output: &str) -> Option<String> {
    let re = regex::Regex::new(r"sing-box\s+version\s+v?([0-9][0-9A-Za-z.\-]*)").ok()?;
    // Subcommand output may contain multiple lines: prefer first line containing "version"
    // (sing-box 1.14+), fallback to full text match.
    if let Some(line) = output.lines().find(|l| l.contains("version"))
        && let Some(m) = re.captures(line).and_then(|c| c.get(1))
    {
        return Some(m.as_str().to_string());
    }
    re.captures(output)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

/// Verify version probe output contains target version (allow `v` prefix, or parsed version equal).
pub(super) fn verify_version(binary: &Path, version: &str) -> PanelResult<()> {
    let text = binary_output(binary);
    let parsed = parse_version_from_output(&text).unwrap_or_default();
    if !version.is_empty()
        && (text.contains(version) || text.contains(&format!("v{version}")) || parsed == version)
    {
        return Ok(());
    }
    Err(PanelError::Core(format!(
        "Core sing-box version verification failed: requested {version}, version probe output: {text}"
    )))
}

/// Semantic version comparison (for [`ClientCoreInventory::preferred_binary`] sorting downloaded cores).
///
/// Rules:
/// - Numeric segments (`.` separated) compared numerically: `1.14.0` > `1.13.15`;
/// - Versions with same numeric segments but prerelease suffix are lower than stable
///   (`1.14.0-beta.4` < `1.14.0`);
/// - Strings that cannot be parsed into version segments (e.g. `unknown`) are treated as empty
///   segments → oldest.
pub(super) fn compare_core_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let (a_pre, a_core) = split_version_identity(a);
    let (b_pre, b_core) = split_version_identity(b);

    let a_nums = parse_numeric_segments(&a_core);
    let b_nums = parse_numeric_segments(&b_core);
    // Compare numeric segments; shorter one is smaller when numerically equal (1.14 < 1.14.0).
    for (x, y) in a_nums.iter().zip(b_nums.iter()) {
        match x.cmp(y) {
            std::cmp::Ordering::Equal => {}
            other => return other,
        }
    }
    let len_cmp = a_nums.len().cmp(&b_nums.len());
    if len_cmp != std::cmp::Ordering::Equal {
        return len_cmp;
    }
    // Numeric segments equal: stable > prerelease; prereleases compared by marker + tail.
    match (a_pre, b_pre) {
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, None) => std::cmp::Ordering::Equal,
        (Some(p), Some(q)) => p.cmp(&q).then_with(|| a_core.cmp(&b_core)),
    }
}

/// Split version into (prerelease prefix, numeric core).
///
/// `1.14.0-beta.4` → `(Some("beta.4"), "1.14.0")`;
/// `1.13.15` → `(None, "1.13.15")`.
fn split_version_identity(v: &str) -> (Option<String>, String) {
    let v = v.trim();
    // Standard `number[.number].prerelease`: after `-` is prerelease marker.
    if let Some(idx) = v.find('-') {
        let (core, pre) = v.split_at(idx);
        let pre = pre.trim_start_matches('-');
        if !core.is_empty() && core.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            return (Some(pre.to_ascii_lowercase()), core.to_string());
        }
    }
    (None, v.to_string())
}

/// Parse `.` separated numeric segments (non-numeric segments truncated and ignored).
fn parse_numeric_segments(s: &str) -> Vec<u64> {
    s.split('.')
        .map(|seg| seg.trim_end_matches(|c: char| !c.is_ascii_digit()))
        .filter(|seg| !seg.is_empty())
        .filter_map(|seg| seg.parse::<u64>().ok())
        .collect()
}

#[cfg(test)]
mod version_tests {
    use super::compare_core_versions;
    use std::cmp::Ordering;

    #[test]
    fn compares_numeric_segments() {
        assert_eq!(compare_core_versions("1.13.15", "1.14.0"), Ordering::Less);
        assert_eq!(
            compare_core_versions("1.14.0", "1.13.15"),
            Ordering::Greater
        );
        assert_eq!(compare_core_versions("1.14.0", "1.14.0"), Ordering::Equal);
        // Trailing segment semantics: 1.14 < 1.14.0.
        assert_eq!(compare_core_versions("1.14", "1.14.0"), Ordering::Less);
    }

    #[test]
    fn prerelease_sorts_below_same_base_stable() {
        assert_eq!(
            compare_core_versions("1.14.0-beta.4", "1.14.0"),
            Ordering::Less
        );
        assert_eq!(
            compare_core_versions("1.14.0", "1.14.0-beta.4"),
            Ordering::Greater
        );
        // Prerelease base version higher than old stable: 1.14.0-beta.4 > 1.13.15.
        assert_eq!(
            compare_core_versions("1.13.15", "1.14.0-beta.4"),
            Ordering::Less
        );
    }

    #[test]
    fn unknown_versions_fallback_lowest() {
        // Strings that cannot be parsed into version segments (e.g. `unknown`) are treated as empty
        // segments → oldest; two unknowns are equal.
        assert_eq!(compare_core_versions("unknown", "1.19.29"), Ordering::Less);
        assert_eq!(compare_core_versions("unknown", "unknown"), Ordering::Equal);
    }
}
