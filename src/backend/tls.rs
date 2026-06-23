use std::path::PathBuf;

use rcgen::{CertificateParams, DnType, IsCa, Issuer, KeyPair, SanType};
use tracing::info;

/// Directory for TLS certificates under the config directory.
fn tls_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_default()
        .join("llm-manager")
        .join("tls")
}

/// Well-known CA certificate path.
fn ca_cert_path() -> PathBuf {
    tls_dir().join("ca.pem")
}

/// Well-known CA private key path.
fn ca_key_path() -> PathBuf {
    tls_dir().join("ca-key.pem")
}

/// Well-known server certificate path.
fn server_cert_path() -> PathBuf {
    tls_dir().join("server.pem")
}

/// Well-known server private key path.
fn server_key_path() -> PathBuf {
    tls_dir().join("server-key.pem")
}

/// TLS version file path — triggers regeneration when bumped.
fn tls_version_path() -> PathBuf {
    tls_dir().join("version")
}

/// TLS version string stored in the version file.
const TLS_VERSION: &str = "1";

/// Load TLS config from PEM files.
pub async fn load_tls_config(
    cert_path: &str,
    key_path: &str,
) -> Result<axum_server::tls_rustls::RustlsConfig, Box<dyn std::error::Error + Send + Sync>> {
    info!("Loading TLS config from {} and {}", cert_path, key_path);
    axum_server::tls_rustls::RustlsConfig::from_pem_file(cert_path, key_path)
        .await
        .map_err(|e| e.into())
}

/// Generate a self-signed CA certificate and key, persisting them to disk.
/// Returns (cert_pem, key_pem).
fn generate_ca() -> Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
    let key = KeyPair::generate()?;
    let mut params = CertificateParams::default();
    params.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    params
        .distinguished_name
        .push(DnType::CommonName, "llm-manager CA".to_string());
    let cert = params.self_signed(&key)?;
    let cert_pem = cert.pem();
    let key_pem = key.serialize_pem();
    Ok((cert_pem, key_pem))
}

/// Parse CA certificate and key from PEM strings.
fn parse_ca_from_pem(
    ca_cert_pem: &str,
    ca_key_pem: &str,
) -> Result<(Issuer<'static, KeyPair>, KeyPair), Box<dyn std::error::Error + Send + Sync>> {
    let ca_key1 = KeyPair::from_pem(ca_key_pem)?;
    let ca_issuer = Issuer::from_ca_cert_pem(ca_cert_pem, ca_key1)?;
    let ca_key2 = KeyPair::from_pem(ca_key_pem)?;
    Ok((ca_issuer, ca_key2))
}

/// Generate a server certificate signed by the CA, persisting to disk.
/// The cert includes the given host alongside localhost/127.0.0.1 in SANs.
/// Returns (cert_pem, key_pem).
fn generate_server_cert(
    ca_cert_pem: &str,
    ca_key_pem: &str,
    host: &str,
) -> Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
    let server_key = KeyPair::generate()?;

    let (ca_issuer, _ca_key) = parse_ca_from_pem(ca_cert_pem, ca_key_pem)?;

    // Generate server cert signed by CA
    let mut params = CertificateParams::default();
    let mut san_entries = vec![
        SanType::DnsName("localhost".try_into().unwrap()),
        SanType::IpAddress([127, 0, 0, 1].into()),
    ];
    if host != "localhost" {
        if let Ok(ip) = host.parse::<std::net::IpAddr>() {
            san_entries.push(SanType::IpAddress(ip));
        } else {
            san_entries.push(SanType::DnsName(host.try_into().unwrap()));
        }
    }
    params.subject_alt_names = san_entries;
    let cert = params.signed_by(&server_key, &ca_issuer)?;
    let cert_pem = cert.pem();
    let key_pem = server_key.serialize_pem();
    Ok((cert_pem, key_pem))
}

/// Ensure TLS certificates exist. If not, generates a CA + server cert pair.
/// The server cert is issued for the given host alongside localhost/127.0.0.1.
/// Returns the paths to the cert and key files.
pub fn ensure_tls_certs(host: &str) -> Result<(PathBuf, PathBuf), Box<dyn std::error::Error + Send + Sync>> {
    let ca_path = ca_cert_path();
    let ca_key_path = ca_key_path();
    let server_cert_path = server_cert_path();
    let server_key_path = server_key_path();
    let version_path = tls_version_path();

    // If server cert exists AND version matches, return it
    let version_matches = version_path.exists()
        && std::fs::read_to_string(&version_path).ok().as_deref() == Some(TLS_VERSION);
    if server_cert_path.exists() && server_key_path.exists() && version_matches
        && try_load_tls(
            server_cert_path.to_str().unwrap(),
            server_key_path.to_str().unwrap(),
        )
        .is_ok()
    {
        return Ok((server_cert_path, server_key_path));
    }
    // Certs corrupt or version mismatch — fall through to regenerate

    // Create TLS directory
    std::fs::create_dir_all(tls_dir())?;

    // Check CA expiry if CA already exists
    if ca_path.exists() && ca_key_path.exists() {
        let ca_pem = std::fs::read_to_string(&ca_path)?;
        let (_, cert) = x509_parser::parse_x509_certificate(ca_pem.as_bytes())?;
        let not_after = cert.validity().not_after;
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let expiry_secs = not_after.timestamp() as u64;
        let six_months = 183 * 24 * 3600;
        if expiry_secs > now_secs && expiry_secs - now_secs < six_months {
            let days_left = (expiry_secs - now_secs) / 86400;
            info!(
                "CA certificate expires in {} days. Consider regenerating TLS certs.",
                days_left
            );
        }
    }

    // Generate or load CA
    let (ca_cert_pem, ca_key_pem) = if ca_path.exists() && ca_key_path.exists() {
        (
            std::fs::read_to_string(&ca_path)?,
            std::fs::read_to_string(&ca_key_path)?,
        )
    } else {
        let (cert, key) = generate_ca()?;
        std::fs::write(&ca_path, &cert)?;
        std::fs::write(&ca_key_path, &key)?;
        (cert, key)
    };

    // Generate server cert signed by CA (use the in-memory CA cert, don't re-read from disk)
    let (server_cert, server_key) = generate_server_cert(&ca_cert_pem, &ca_key_pem, host)?;
    // Write to temp files first, then rename for atomicity
    let tmp_cert_path = server_cert_path.with_extension("pem.tmp");
    let tmp_key_path = server_key_path.with_extension("pem.tmp");
    std::fs::write(&tmp_cert_path, &server_cert)?;
    std::fs::write(&tmp_key_path, &server_key)?;
    std::fs::rename(&tmp_cert_path, &server_cert_path)?;
    std::fs::rename(&tmp_key_path, &server_key_path)?;

    // Validate just-written certs immediately; delete and regenerate on failure
    // to avoid leaving the user with corrupt files that pass the version check.
    if try_load_tls(
        server_cert_path.to_str().unwrap(),
        server_key_path.to_str().unwrap(),
    )
    .is_err()
    {
        tracing::warn!("Generated TLS certs failed validation, removing for regeneration");
        let _ = std::fs::remove_file(&server_cert_path);
        let _ = std::fs::remove_file(&server_key_path);
        let _ = std::fs::remove_file(&version_path);
        return ensure_tls_certs("localhost");
    }

    // Write version file
    std::fs::write(&version_path, TLS_VERSION)?;

    info!(
        "Generated self-signed TLS certificates in {}",
        tls_dir().display()
    );
    info!(
        "To trust this CA, install {} into your system trust store:",
        ca_path.display()
    );
    info!(
        "  Linux (system-wide): sudo cp {} /usr/local/share/ca-certificates/ && sudo update-ca-certificates",
        ca_path.display()
    );
    info!(
        "  macOS:             sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain {}",
        ca_path.display()
    );

    Ok((server_cert_path, server_key_path))
}

/// Check if a path is valid for TLS (exists and is a file).
pub fn validate_tls_path(path: &str) -> Result<(), String> {
    let p = PathBuf::from(path);
    if !p.exists() {
        return Err(format!("TLS file not found: {}", path));
    }
    if !p.is_file() {
        return Err(format!("TLS path is not a file: {}", path));
    }
    Ok(())
}

/// Attempt to load TLS config from PEM files. Returns Ok if valid.
pub fn try_load_tls(cert_path: &str, key_path: &str) -> Result<(), String> {
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current()
            .block_on(async {
                axum_server::tls_rustls::RustlsConfig::from_pem_file(cert_path, key_path)
                    .await
                    .map_err(|e| format!("TLS load failed: {e}"))
            })
            .map(|_| ())
    })
}
