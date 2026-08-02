//! MITM CA 证书管理。
//!
//! 通过 [`CaStore`] 抽象 CA 材料（证书 + 私钥）的加载与持久化，
//! [`FileCaStore`] 为基于本地目录的默认实现：首次调用时用 rcgen 生成
//! 自签 CA 并落盘（Unix 0600 权限），后续调用直接加载既有材料。

use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use pp_common::error::{PanelError, PanelResult};

/// MITM CA 材料（PEM 编码）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaMaterial {
    pub cert_pem: String,
    pub key_pem: String,
}

/// CA 材料的存储抽象。
pub trait CaStore: Send + Sync {
    /// 从存储加载既有 CA，若不存在则生成并持久化新的自签 CA。
    fn load_or_generate(&self) -> PanelResult<CaMaterial>;
}

/// 基于本地目录的 [`CaStore`]。
///
/// 目录内约定 `ca.crt` / `ca.key` 两个文件。任一文件缺失时生成新的
/// 自签 CA 并以 Unix 0600 权限写入；后续加载直接返回持久化材料。
#[derive(Debug, Clone)]
pub struct FileCaStore {
    dir: PathBuf,
}

impl FileCaStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn cert_path(&self) -> PathBuf {
        self.dir.join("ca.crt")
    }

    fn key_path(&self) -> PathBuf {
        self.dir.join("ca.key")
    }
}

impl CaStore for FileCaStore {
    fn load_or_generate(&self) -> PanelResult<CaMaterial> {
        fs::create_dir_all(&self.dir)
            .map_err(|e| PanelError::Mitm(format!("create ca dir {:?}: {e}", self.dir)))?;
        let cert_path = self.cert_path();
        let key_path = self.key_path();
        if cert_path.exists() && key_path.exists() {
            return load_material(&cert_path, &key_path);
        }
        let material = generate_ca_material()?;
        write_private(&cert_path, &material.cert_pem)?;
        write_private(&key_path, &material.key_pem)?;
        // 二次加载校验一致：重新从磁盘读取并返回，确保落盘内容与生成内容一致。
        load_material(&cert_path, &key_path)
    }
}

fn load_material(cert_path: &Path, key_path: &Path) -> PanelResult<CaMaterial> {
    let cert_pem = fs::read_to_string(cert_path)
        .map_err(|e| PanelError::Mitm(format!("read ca cert {:?}: {e}", cert_path)))?;
    let key_pem = fs::read_to_string(key_path)
        .map_err(|e| PanelError::Mitm(format!("read ca key {:?}: {e}", key_path)))?;
    Ok(CaMaterial { cert_pem, key_pem })
}

/// 构造自签 CA 的证书参数。
///
/// - SAN 留空（名称字符串曾误当 SAN 传入，导致生成出空 DN 的证书）
/// - CN 经 distinguished_name 显式设置
/// - `IsCa::Ca(Unconstrained)` + key usage 含 keyCertSign / cRLSign / digitalSignature
/// - 显式有效期 10 年（not_before 回拨 1 天缓冲时钟偏差）
fn ca_params() -> PanelResult<rcgen::CertificateParams> {
    // rcgen 0.14：`CertificateParams::new` 的参数是 SAN 列表。
    let mut params = rcgen::CertificateParams::new(Vec::<String>::new())
        .map_err(|e| PanelError::Mitm(format!("init ca params: {e}")))?;
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "ProxyPanel MITM CA");
    params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    // 显式声明 CA 用途：keyCertSign / cRLSign / digitalSignature。
    // 缺 keyCertSign 时 iOS/macOS 及部分 Android 严格校验会拒绝该 CA
    // （TLS 握手 `CertificateUnknown`）。
    params.key_usages = vec![
        rcgen::KeyUsagePurpose::KeyCertSign,
        rcgen::KeyUsagePurpose::CrlSign,
        rcgen::KeyUsagePurpose::DigitalSignature,
    ];
    // 显式有效期：10 年，not_before 回拨 1 天缓冲客户端时钟偏差。
    let now = time::OffsetDateTime::now_utc();
    params.not_before = now - time::Duration::days(1);
    params.not_after = now + time::Duration::days(3650);
    Ok(params)
}

fn generate_ca_material() -> PanelResult<CaMaterial> {
    let key_pair = rcgen::KeyPair::generate()
        .map_err(|e| PanelError::Mitm(format!("generate ca key pair: {e}")))?;
    let cert = ca_params()?
        .self_signed(&key_pair)
        .map_err(|e| PanelError::Mitm(format!("self sign ca: {e}")))?;
    Ok(CaMaterial {
        cert_pem: cert.pem(),
        key_pem: key_pair.serialize_pem(),
    })
}

/// 以 Unix 0600 权限写入文件（创建/覆盖）。
fn write_private(path: &Path, contents: &str) -> PanelResult<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)
        .map_err(|e| PanelError::Mitm(format!("create {:?}: {e}", path)))?;
    file.write_all(contents.as_bytes())
        .map_err(|e| PanelError::Mitm(format!("write {:?}: {e}", path)))?;
    file.sync_all()
        .map_err(|e| PanelError::Mitm(format!("sync {:?}: {e}", path)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    #[test]
    fn file_ca_store_generates_and_reloads_consistently() {
        let dir = tempdir().unwrap();
        let store = FileCaStore::new(dir.path());
        let first = store.load_or_generate().unwrap();
        let second = store.load_or_generate().unwrap();
        assert_eq!(first.cert_pem, second.cert_pem);
        assert_eq!(first.key_pem, second.key_pem);
        assert!(first.cert_pem.contains("BEGIN CERTIFICATE"));
        assert!(first.key_pem.contains("PRIVATE KEY"));
        assert!(!first.key_pem.contains("PUBLIC KEY"));
    }

    #[test]
    fn file_ca_store_writes_private_files_with_0600() {
        let dir = tempdir().unwrap();
        let store = FileCaStore::new(dir.path());
        store.load_or_generate().unwrap();
        for name in ["ca.crt", "ca.key"] {
            let path = dir.path().join(name);
            let mode = fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "unexpected mode for {name}: {mode:o}");
        }
    }

    /// 生成的 CA 必须能通过 hudsucker 的 `Issuer::from_ca_cert_pem` 解析，并真实
    /// 签发叶证书。这是 keyCertSign / is_ca 修正的回归测试：旧 CA（缺 key usage、
    /// 空 DN）在严格客户端（iOS/macOS）TLS 握手中会被拒绝（CertificateUnknown）。
    #[test]
    fn generated_ca_parses_with_hudsucker_issuer_and_signs_leaf() {
        let material = generate_ca_material().unwrap();
        let key_pair = hudsucker::rcgen::KeyPair::from_pem(&material.key_pem).unwrap();
        let issuer = hudsucker::rcgen::Issuer::from_ca_cert_pem(&material.cert_pem, key_pair)
            .expect("CA 材料应能被 hudsucker Issuer 解析");

        // 用该 CA 真实签发一个叶证书：解析 + 签发全链路通过即证明 CA 可用。
        let leaf_key = hudsucker::rcgen::KeyPair::generate().unwrap();
        let leaf =
            hudsucker::rcgen::CertificateParams::new(vec!["example.com".to_string()]).unwrap();
        leaf.signed_by(&leaf_key, &issuer)
            .expect("CA 应能签发叶证书");
    }

    /// 生成的 CA 携带正确的 CN / key usage / CA 约束 / 有效期（本次修复的核心点）。
    #[test]
    fn ca_params_carry_cn_key_usage_and_validity() {
        let params = ca_params().unwrap();
        assert_eq!(
            params.distinguished_name.get(&rcgen::DnType::CommonName),
            Some(&rcgen::DnValue::Utf8String(
                "ProxyPanel MITM CA".to_string()
            ))
        );
        assert_eq!(
            params.key_usages,
            vec![
                rcgen::KeyUsagePurpose::KeyCertSign,
                rcgen::KeyUsagePurpose::CrlSign,
                rcgen::KeyUsagePurpose::DigitalSignature,
            ]
        );
        assert!(matches!(
            params.is_ca,
            rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained)
        ));
        // not_before 回拨 1 天（早于当前时间），有效期约 10 年（±2 天容差）。
        assert!(
            params.not_before < time::OffsetDateTime::now_utc(),
            "not_before 应回拨到当前时间之前"
        );
        let span = params.not_after - params.not_before;
        assert!(
            span >= time::Duration::days(3649),
            "有效期不足 10 年: {span:?}"
        );
        assert!(span <= time::Duration::days(3653), "有效期过长: {span:?}");
    }
}
