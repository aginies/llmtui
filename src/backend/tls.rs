use std::path::PathBuf;

use rcgen::{
    CertificateParams, DnType, IsCa, Issuer, KeyPair,
    SanType,
};
use rustls_pemfile::certs;
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
    let cert_file = cert_path;
    let key_file = key_path;

    info!("Loading TLS config from {} and {}", cert_file, key_file);
    axum_server::tls_rustls::RustlsConfig::from_pem_file(cert_file, key_file).await
        .map_err(|e| e.into())
}

/// Generate a self-signed CA certificate and key, persisting them to disk.
/// Returns (cert_pem, key_pem).
fn generate_ca() -> Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
    let key = KeyPair::generate()?;
    let mut params = CertificateParams::default();
    params.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    params.distinguished_name.push(
        DnType::CommonName,
        "llm-manager CA".to_string(),
    );
    let cert = params.self_signed(&key)?;
    let cert_pem = cert.pem();
    let key_pem = key.serialize_pem();
    Ok((cert_pem, key_pem))
}

/// Parse CA certificate and key from PEM strings.
fn parse_ca_from_pem(ca_cert_pem: &str, ca_key_pem: &str) -> Result<(rcgen::Certificate, Issuer<'static, KeyPair>), Box<dyn std::error::Error + Send + Sync>> {
    let ca_cert_der = certs(&mut ca_cert_pem.as_bytes())
        .next()
        .ok_or("No CA certificate found in PEM")?
        .map_err(|e| format!("Failed to parse CA certificate: {e}"))?;
    let ca_cert = rcgen::Certificate::try_from(ca_cert_der)
        .map_err(|e| format!("Failed to convert CA certificate: {e}"))?;

    let ca_key = KeyPair::from_pem(ca_key_pem)?;
    let ca_issuer = Issuer::from((ca_cert, ca_key));
    Ok((ca_cert, ca_issuer))
}

/// Generate a server certificate signed by the CA, persisting to disk.
/// Returns (cert_pem, key_pem).
fn generate_server_cert(ca_cert_pem: &str, ca_key_pem: &str) -> Result<(String, String), Box<dyn std::error::Error + Send + Sync>> {
    let server_key = KeyPair::generate()?;

    let (ca_cert, ca_issuer) = parse_ca_from_pem(ca_cert_pem, ca_key_pem)?;

    // Generate server cert
    let mut params = CertificateParams::default();
    params.subject_alt_names = vec![
        SanType::DnsName("localhost".try_into().unwrap()),
        SanType::IpAddress([127, 0, 0, 1].try_into().unwrap()),
        SanType::IpAddress([0, 0, 0, 0].try_into().unwrap()),
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
    let server_cert_path = server_cert_path();
    let server_key_path = server_key_path();

    // If server cert already exists, return it
    if server_cert_path.exists() && server_key_path.exists() {
        return Ok((server_cert_path, server_key_path));
    }

    // Create TLS directory
    std::fs::create_dir_all(tls_dir())?;

    // Generate or load CA
    let ca_cert_pem = if ca_path.exists() {
        std::fs::read_to_string(&ca_path)?
    } else {
        let (cert, _) = generate_ca()?;
        std::fs::write(&ca_path, &cert)?;
        cert
    };

    // Generate server cert signed by CA
    let (server_cert, server_key) = generate_server_cert(&ca_cert_pem, &std::fs::read_to_string(&ca_path)?)?;
    std::fs::write(&server_cert_path, &server_cert)?;
    std::fs::write(&server_key_path, &server_key)?;

    info!("Generated self-signed TLS certificates in {}", tls_dir().display());
    info!("To trust this CA, install {} into your system trust store:", ca_path.display());
    info!("  Linux (system-wide): sudo cp {} /usr/local/share/ca-certificates/ && sudo update-ca-certificates", ca_path.display());
    info!("  macOS:             sudo security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain {}", ca_path.display());

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
