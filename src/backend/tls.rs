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
/// Returns (cert_pem, key_pem).
fn generate_server_cert(
    ca_cert_pem: &str,
    ca_key_pem: &str,
) -> Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
    let server_key = KeyPair::generate()?;

    let (ca_issuer, _ca_key) = parse_ca_from_pem(ca_cert_pem, ca_key_pem)?;

    // Generate server cert signed by CA
    let mut params = CertificateParams::default();
    params.subject_alt_names = vec![
        SanType::DnsName("localhost".try_into().unwrap()),
        SanType::IpAddress([127, 0, 0, 1].into()),
        SanType::IpAddress([0, 0, 0, 0].into()),
    ];
    let cert = params.signed_by(&server_key, &ca_issuer)?;
    let cert_pem = cert.pem();
    let key_pem = server_key.serialize_pem();
    Ok((cert_pem, key_pem))
}

/// Ensure TLS certificates exist. If not, generates a CA + server cert pair.
/// Returns the paths to the cert and key files.
pub fn ensure_tls_certs() -> Result<(PathBuf, PathBuf), Box<dyn std::error::Error + Send + Sync>> {
    let ca_path = ca_cert_path();
    let ca_key_path = ca_key_path();
    let server_cert_path = server_cert_path();
    let server_key_path = server_key_path();

    // If server cert already exists, return it
    if server_cert_path.exists() && server_key_path.exists() {
        return Ok((server_cert_path, server_key_path));
    }

    // Create TLS directory
    std::fs::create_dir_all(tls_dir())?;

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
    let (server_cert, server_key) = generate_server_cert(&ca_cert_pem, &ca_key_pem)?;
    std::fs::write(&server_cert_path, &server_cert)?;
    std::fs::write(&server_key_path, &server_key)?;

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
