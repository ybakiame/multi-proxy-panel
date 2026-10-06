//! Core process spawn path absolutization.

use std::path::{Path, PathBuf};

/// Absolutize a path against the process current directory.
///
/// Core binaries change their own working directory on startup (sing-box `-D` /
/// mihomo `-d`): a relative binary/config path would be resolved twice against the
/// *new* cwd and fail (observed on Windows where the client data dir fell back to a
/// relative `./.proxy-panel-client` when `HOME` is unset). Resolving eagerly at
/// construction keeps spawn paths stable regardless of later cwd changes.
pub(super) fn absolutize(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
    }
}

#[cfg(test)]
mod path_tests {
    use super::absolutize;
    use std::path::Path;

    #[test]
    fn absolutize_keeps_absolute_paths() {
        #[cfg(windows)]
        let abs = Path::new(r"C:\data\cores\sing-box.exe");
        #[cfg(not(windows))]
        let abs = Path::new("/data/cores/sing-box");
        assert_eq!(absolutize(abs), abs);
    }

    #[test]
    fn absolutize_resolves_relative_against_cwd() {
        let cwd = std::env::current_dir().unwrap();
        let got = absolutize(Path::new(".proxy-panel-client"));
        assert!(got.is_absolute());
        assert_eq!(got, cwd.join(".proxy-panel-client"));
    }
}
