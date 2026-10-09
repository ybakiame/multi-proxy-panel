//! `collect_hostname_sources` 派生白名单来源聚合测试。

use crate::remote::*;
use std::path::Path;

struct TestDir(std::path::PathBuf);

impl TestDir {
    fn new() -> Self {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "pp-client-hostname-sources-{}-{n}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write_cache(dir: &Path, file: &str, hostnames: &[&str]) {
    let cache_dir = dir.join("remote_cache");
    std::fs::create_dir_all(&cache_dir).unwrap();
    let cached = CachedRemoteConfig {
        hostnames: hostnames.iter().map(|s| s.to_string()).collect(),
        ..CachedRemoteConfig::default()
    };
    std::fs::write(
        cache_dir.join(file),
        serde_json::to_string_pretty(&cached).unwrap(),
    )
    .unwrap();
}

#[test]
fn empty_cache_dir_yields_empty_view() {
    let dir = TestDir::new();
    assert!(collect_hostname_sources(dir.path()).is_empty());
}

#[test]
fn aggregates_sources_per_hostname() {
    let dir = TestDir::new();
    let manager = RemoteManager::new(dir.path().to_path_buf());
    manager
        .save(&[RemoteResource {
            name: "去广告".to_string(),
            url: "https://example.com/a.sgmodule".to_string(),
            kind: RemoteKind::Snippet,
            ..RemoteResource::default()
        }])
        .unwrap();
    write_cache(dir.path(), "imported.json", &["a.com", "shared.com"]);
    write_cache(dir.path(), "去广告.json", &["b.com", "shared.com"]);

    let view = collect_hostname_sources(dir.path());
    assert_eq!(view.len(), 3);
    // 按 hostname 排序。
    assert_eq!(view[0].hostname, "a.com");
    assert_eq!(view[0].sources, vec!["本地导入".to_string()]);
    assert_eq!(view[1].hostname, "b.com");
    assert_eq!(view[1].sources, vec!["去广告".to_string()]);
    // 同一 hostname 多来源合并、排序稳定（文件名序：imported < 去广告）。
    assert_eq!(view[2].hostname, "shared.com");
    assert_eq!(
        view[2].sources,
        vec!["本地导入".to_string(), "去广告".to_string()]
    );
}

#[test]
fn unknown_cache_file_falls_back_to_stem() {
    let dir = TestDir::new();
    write_cache(dir.path(), "orphan.json", &["x.com"]);
    let view = collect_hostname_sources(dir.path());
    assert_eq!(view.len(), 1);
    assert_eq!(view[0].sources, vec!["orphan".to_string()]);
}
