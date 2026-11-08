//! macOS only: a local HTTPS endpoint for the PowerPoint add-in.
//!
//! PowerPoint for Mac shows add-ins in a WebKit view that refuses to call
//! plain `http://127.0.0.1` from the add-in's `https://` page. So on Mac the
//! control API is ALSO served on `https://localhost:47616`.
//!
//! The certificate:
//! - is created on this Mac the first time SlideRelay runs,
//! - is valid only for `localhost` / `127.0.0.1` and only for TLS servers,
//! - is NOT a certificate authority, so it can't be used to vouch for any
//!   other website even if someone copied it,
//! - is trusted once by the teacher (macOS asks for their password).
//!
//! The private key never leaves the app's config folder.

use anyhow::Context;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const TLS_PORT: u16 = 47616;

/// Apple rejects TLS certificates valid for more than 825 days.
const VALID_DAYS: i64 = 800;
/// Make a new certificate when the current one has less than this left.
const RENEW_DAYS: i64 = 30;
const COMMON_NAME: &str = "SlideRelay local connection";

#[derive(Clone)]
pub struct LocalCert {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}

impl LocalCert {
    pub fn in_dir(dir: &Path) -> Self {
        Self {
            cert_path: dir.join("localhost-cert.pem"),
            key_path: dir.join("localhost-key.pem"),
        }
    }

    /// Creates the certificate if it is missing or about to expire.
    pub fn ensure(&self) -> anyhow::Result<()> {
        if self.is_fresh() {
            return Ok(());
        }
        let mut params = rcgen::CertificateParams::new(vec!["localhost".to_string()])?;
        params
            .subject_alt_names
            .push(rcgen::SanType::IpAddress([127, 0, 0, 1].into()));
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, COMMON_NAME);
        params.is_ca = rcgen::IsCa::ExplicitNoCa;
        params.key_usages = vec![rcgen::KeyUsagePurpose::DigitalSignature];
        params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];
        let now = time::OffsetDateTime::now_utc();
        params.not_before = now - time::Duration::days(1);
        params.not_after = now + time::Duration::days(VALID_DAYS);

        let key = rcgen::KeyPair::generate()?;
        let cert = params.self_signed(&key)?;

        if let Some(dir) = self.cert_path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        write_private(&self.key_path, key.serialize_pem().as_bytes())?;
        std::fs::write(&self.cert_path, cert.pem())?;
        tracing::info!("created a new local HTTPS certificate");
        Ok(())
    }

    /// True if macOS already trusts our certificate for `localhost`.
    pub fn is_trusted(&self) -> bool {
        Command::new("/usr/bin/security")
            .args(["verify-cert", "-q", "-p", "ssl", "-s", "localhost", "-c"])
            .arg(&self.cert_path)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Asks macOS to trust the certificate. macOS shows its own password
    /// prompt; this blocks until the teacher answers.
    pub fn trust(&self) -> anyhow::Result<()> {
        let keychain = dirs_home()?.join("Library/Keychains/login.keychain-db");
        let output = Command::new("/usr/bin/security")
            .args(["add-trusted-cert", "-r", "trustRoot", "-p", "ssl", "-k"])
            .arg(&keychain)
            .arg(&self.cert_path)
            .output()
            .context("could not run the macOS security tool")?;
        if !output.status.success() {
            anyhow::bail!(
                "macOS did not trust the certificate: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(())
    }

    /// The certificate exists and has more than RENEW_DAYS left.
    fn is_fresh(&self) -> bool {
        if !self.key_path.exists() {
            return false;
        }
        let Ok(meta) = std::fs::metadata(&self.cert_path) else {
            return false;
        };
        // The certificate is only ever written by `ensure`, so its file age
        // tells us how old it is.
        let age_days = meta
            .modified()
            .ok()
            .and_then(|m| m.elapsed().ok())
            .map(|d| d.as_secs() as i64 / 86_400)
            .unwrap_or(i64::MAX);
        age_days < VALID_DAYS - RENEW_DAYS
    }
}

/// Serves `app` over HTTPS on 127.0.0.1:47616 using the local certificate.
pub async fn serve(cert: &LocalCert, app: axum::Router) -> anyhow::Result<()> {
    // rustls needs one crypto provider chosen for the whole process.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let config =
        axum_server::tls_rustls::RustlsConfig::from_pem_file(&cert.cert_path, &cert.key_path)
            .await?;
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], TLS_PORT));
    tracing::info!("control API (HTTPS) listening on {addr}");
    axum_server::bind_rustls(addr, config)
        .serve(app.into_make_service())
        .await?;
    Ok(())
}

fn dirs_home() -> anyhow::Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME is not set")
}

/// Writes a file readable only by the current user (the private key).
fn write_private(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    Ok(())
}
