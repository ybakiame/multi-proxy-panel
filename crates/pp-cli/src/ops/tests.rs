//! Unit tests for ops modules.

#[test]
fn build_download_url_latest() {
    let url = super::build_download_url("owner/repo", "latest", "asset.tar.gz");
    assert_eq!(
        url,
        "https://github.com/owner/repo/releases/latest/download/asset.tar.gz"
    );
}

#[test]
fn build_download_url_tagged() {
    let url = super::build_download_url("owner/repo", "v1.2.3", "asset.tar.gz");
    assert_eq!(
        url,
        "https://github.com/owner/repo/releases/download/v1.2.3/asset.tar.gz"
    );
}

#[test]
fn build_download_url_tagged_no_v() {
    let url = super::build_download_url("owner/repo", "1.2.3", "asset.tar.gz");
    assert_eq!(
        url,
        "https://github.com/owner/repo/releases/download/v1.2.3/asset.tar.gz"
    );
}

#[test]
fn parse_sha256_from_sums_ok() {
    let sums = "abc123  file.tar.gz\ndef456  other.tar.gz\n";
    assert_eq!(
        super::download::parse_sha256_from_sums(sums, "file.tar.gz"),
        Some("abc123".to_string())
    );
    assert_eq!(
        super::download::parse_sha256_from_sums(sums, "other.tar.gz"),
        Some("def456".to_string())
    );
    assert_eq!(super::download::parse_sha256_from_sums(sums, "missing.tar.gz"), None);
}

#[test]
fn parse_sha256_skips_comments_and_empty() {
    let sums = "# comment\n\nabc123  file.tar.gz\n";
    assert_eq!(
        super::download::parse_sha256_from_sums(sums, "file.tar.gz"),
        Some("abc123".to_string())
    );
}

#[test]
fn parse_version_from_output_ok() {
    assert_eq!(
        super::download::parse_version_from_output("proxy-panel 0.3.3"),
        Some("0.3.3".to_string())
    );
    assert_eq!(
        super::download::parse_version_from_output("proxy-panel-hub 0.3.3\n"),
        Some("0.3.3".to_string())
    );
    assert_eq!(
        super::download::parse_version_from_output("some-tool 1.0.0-beta.2"),
        Some("1.0.0-beta.2".to_string())
    );
}

#[test]
fn parse_version_from_output_empty() {
    assert_eq!(super::download::parse_version_from_output(""), None);
    assert_eq!(super::download::parse_version_from_output("\n"), None);
}

#[test]
fn backup_path_format() {
    use std::path::PathBuf;
    let p = super::fsutil::backup_path("proxy-panel-hub", "0.3.3");
    assert_eq!(
        p,
        PathBuf::from("/opt/proxy-panel/backup/proxy-panel-hub.0.3.3.bak")
    );
}
