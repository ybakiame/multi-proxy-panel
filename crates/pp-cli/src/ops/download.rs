//! HTTP download helpers and SHA-256 verification.

use anyhow::{Context, Result, bail};
use futures::StreamExt;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

use super::build_download_url;

/// Parse the expected SHA-256 hash for a given asset from the SHA256SUMS content.
pub fn parse_sha256_from_sums(sums_text: &str, asset_name: &str) -> Option<String> {
    for line in sums_text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Format: <hash>  <filename>  (two spaces typical, but allow any whitespace)
        let mut parts = line.split_whitespace();
        let (Some(hash), Some(filename)) = (parts.next(), parts.next()) else {
            continue;
        };
        if filename == asset_name {
            return Some(hash.to_lowercase());
        }
    }
    None
}

/// Compute SHA-256 of a file in a streaming fashion.
pub async fn sha256_file(path: &Path) -> Result<String> {
    let mut file = tokio::fs::File::open(path)
        .await
        .with_context(|| format!("failed to open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 8192];
    loop {
        let n = file
            .read(&mut buf)
            .await
            .with_context(|| format!("failed to read {}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Parse version string from `--version` stdout like "proxy-panel 0.3.3".
pub fn parse_version_from_output(output: &str) -> Option<String> {
    // Take the first line, then the last whitespace-separated token.
    let first = output.lines().next()?.trim();
    first.split_whitespace().last().map(|s| s.to_string())
}

/// Download text content from a URL.
pub async fn download_text(client: &reqwest::Client, url: &str) -> Result<String> {
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("failed to GET {}", url))?;
    let status = resp.status();
    if !status.is_success() {
        bail!("下载失败 {}: HTTP {}", url, status);
    }
    resp.text()
        .await
        .with_context(|| format!("failed to read body from {}", url))
}

/// Download binary content to a file.
pub async fn download_to_file(client: &reqwest::Client, url: &str, dest: &Path) -> Result<()> {
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("failed to GET {}", url))?;
    let status = resp.status();
    if !status.is_success() {
        bail!("下载失败 {}: HTTP {}", url, status);
    }
    let mut file = tokio::fs::File::create(dest)
        .await
        .with_context(|| format!("failed to create {}", dest.display()))?;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.with_context(|| format!("download stream error from {}", url))?;
        file.write_all(&chunk)
            .await
            .with_context(|| format!("failed to write to {}", dest.display()))?;
    }
    Ok(())
}

/// Download an asset and its SHA256SUMS, verify hash, return the local file path.
pub async fn download_and_verify(
    client: &reqwest::Client,
    repo: &str,
    version: &str,
    asset: &str,
    tmp_dir: &Path,
) -> Result<PathBuf> {
    let sums_url = build_download_url(repo, version, "SHA256SUMS");
    let sums_text = download_text(client, &sums_url).await?;
    let expected = parse_sha256_from_sums(&sums_text, asset)
        .with_context(|| format!("在 SHA256SUMS 中未找到 {}", asset))?;

    let asset_url = build_download_url(repo, version, asset);
    let asset_path = tmp_dir.join(asset);
    download_to_file(client, &asset_url, &asset_path).await?;

    let actual = sha256_file(&asset_path).await?;
    if actual != expected {
        bail!(
            "SHA-256 校验失败: {}\n期望: {}\n实际: {}",
            asset,
            expected,
            actual
        );
    }

    Ok(asset_path)
}

/// Get the installed version of a binary by running `<bin> --version`.
pub async fn installed_version(bin_path: &Path) -> Result<Option<String>> {
    if !bin_path.exists() {
        return Ok(None);
    }
    let out = Command::new(bin_path)
        .arg("--version")
        .output()
        .await
        .with_context(|| format!("failed to run {} --version", bin_path.display()))?;
    if !out.status.success() {
        return Ok(None);
    }
    let text = String::from_utf8_lossy(&out.stdout);
    Ok(parse_version_from_output(&text))
}
