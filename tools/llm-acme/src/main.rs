use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use lers::{Directory, Format, solver::Solver};
use lers::solver::{Http01Solver, dns::CloudflareDns01Solver};
use openssl::pkey::PKey;
use tracing::info;

const HTTP_LISTEN_ADDR: &str = "0.0.0.0:80";
const PROD_URL: &str = lers::LETS_ENCRYPT_PRODUCTION_URL;
const STAGING_URL: &str = lers::LETS_ENCRYPT_STAGING_URL;

#[derive(Parser)]
#[command(name = "llm-acme", about = "Let's Encrypt certificate management for llm-manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Issue a new certificate for a domain
    Issue {
        /// Domain name to get certificate for
        #[arg(short, long)]
        domain: String,

        /// Email address for Let's Encrypt account (overrides config)
        #[arg(long)]
        email: Option<String>,

        /// Use Let's Encrypt staging server (for testing)
        #[arg(long)]
        staging: bool,

        /// DNS challenge solver type (default: auto-detect from domain)
        #[arg(long)]
        solver: Option<SolverType>,

        /// Cloudflare API token for DNS-01 challenge
        #[arg(long, env = "CLOUDFLARE_API_TOKEN")]
        dns_token: Option<String>,
    },

    /// Renew an existing certificate for a domain
    Renew {
        /// Domain name to renew certificate for
        #[arg(short, long)]
        domain: String,

        /// Email address for Let's Encrypt account (overrides config)
        #[arg(long)]
        email: Option<String>,

        /// Use Let's Encrypt staging server (for testing)
        #[arg(long)]
        staging: bool,

        /// DNS challenge solver type (default: auto-detect from domain)
        #[arg(long)]
        solver: Option<SolverType>,

        /// Cloudflare API token for DNS-01 challenge
        #[arg(long, env = "CLOUDFLARE_API_TOKEN")]
        dns_token: Option<String>,
    },

    /// Revoke a certificate for a domain
    Revoke {
        /// Domain name to revoke certificate for
        #[arg(short, long)]
        domain: String,

        /// Email address for Let's Encrypt account (overrides config)
        #[arg(long)]
        email: Option<String>,

        /// Use Let's Encrypt staging server (for testing)
        #[arg(long)]
        staging: bool,
    },

    /// List all stored certificates and their expiry dates
    List,
}

#[derive(ValueEnum, Clone, Debug)]
enum SolverType {
    /// HTTP-01 challenge (requires port 80 accessible)
    Http,
    /// DNS-01 challenge (requires Cloudflare API token)
    Dns,
}

fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(".config"))
        .join("llm-manager")
}

fn tls_dir() -> PathBuf {
    config_dir().join("tls")
}

fn letsencrypt_dir() -> PathBuf {
    tls_dir().join("letsencrypt")
}

fn domain_dir(domain: &str) -> PathBuf {
    letsencrypt_dir().join(domain)
}

fn cert_path(domain: &str) -> PathBuf {
    domain_dir(domain).join("cert.pem")
}

fn key_path(domain: &str) -> PathBuf {
    domain_dir(domain).join("key.pem")
}

fn account_key_path(domain: &str) -> PathBuf {
    domain_dir(domain).join("account-key.pem")
}

fn config_path() -> PathBuf {
    config_dir().join("config.yaml")
}

fn load_email_from_config() -> Option<String> {
    let cp = config_path();
    if !cp.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&cp).ok()?;
    let config: serde_yml::Value = serde_yml::from_str(&content).ok()?;
    config.get("letsencrypt_email").and_then(|v| v.as_str()).map(String::from)
}

fn get_email(cli_email: Option<String>) -> Result<String> {
    cli_email
        .or_else(load_email_from_config)
        .ok_or_else(|| anyhow::anyhow!(
            "No email provided. Use --email flag or set 'letsencrypt_email' in config.yaml"
        ))
}

fn get_directory_url(staging: bool) -> &'static str {
    if staging {
        STAGING_URL
    } else {
        PROD_URL
    }
}

/// Validate domain name format before ACME call.
fn validate_domain(domain: &str) -> Result<()> {
    if domain.is_empty() {
        bail!("Domain name cannot be empty");
    }
    if domain.len() > 253 {
        bail!("Domain name too long (max 253 characters)");
    }
    for part in domain.split('.') {
        if part.is_empty() {
            bail!("Domain contains empty label (consecutive dots or trailing dot)");
        }
        if part.len() > 63 {
            bail!("Domain label too long: '{}' (max 63 characters)", part);
        }
        for ch in part.chars() {
            if !ch.is_ascii_alphanumeric() && ch != '-' {
                bail!("Domain contains invalid character: '{}'", ch);
            }
        }
        if part.starts_with('-') || part.ends_with('-') {
            bail!("Domain label cannot start or end with hyphen: '{}'", part);
        }
    }
    Ok(())
}

/// Write a file with restricted permissions (owner read/write only).
fn write_private(path: &PathBuf, data: &[u8]) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let tmp = path.with_extension("pem.tmp");
        let mut opts = std::fs::OpenOptions::new();
        opts.create(true).write(true).truncate(true).mode(0o600);
        let mut file = opts.open(&tmp)?;
        std::io::Write::write_all(&mut file, data)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let tmp = path.with_extension("pem.tmp");
        std::fs::write(&tmp, data)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }
}

/// Save cert and key with proper permissions.
fn save_cert(domain: &str, fullchain_pem: &[u8], key_pem: &[u8]) -> Result<()> {
    let dir = domain_dir(domain);
    std::fs::create_dir_all(&dir)?;

    let cp = cert_path(domain);
    let tmp_cert = dir.join("cert.pem.tmp");
    std::fs::write(&tmp_cert, fullchain_pem)?;
    std::fs::rename(&tmp_cert, &cp)?;

    write_private(&key_path(domain), key_pem)?;

    Ok(())
}

/// Check that we can bind to port 80 (requires root or CAP_NET_BIND_SERVICE).
fn check_port_80() {
    #[cfg(unix)]
    {
        if unsafe { libc::geteuid() } != 0 {
            eprintln!("Warning: not running as root. Binding to port 80 may fail.");
            eprintln!("Run with 'sudo' or use: sudo setcap cap_net_bind_service=+ep <binary>");
        }
    }
    #[cfg(not(unix))]
    {
        // On non-Unix, always warn
        eprintln!("Warning: port 80 binding may require elevated privileges.");
    }
}

async fn create_or_load_account(
    directory: &Directory,
    email: &str,
    domain: &str,
) -> Result<lers::Account> {
    let akp = account_key_path(domain);
    let account = if akp.exists() {
        let pem_data = std::fs::read_to_string(&akp)
            .context(format!("Failed to read account key from {}", akp.display()))?;
        let key = PKey::private_key_from_pem(pem_data.as_bytes())
            .context("Failed to parse account private key")?;
        directory
            .account()
            .private_key(key)
            .terms_of_service_agreed(true)
            .contacts(vec![format!("mailto:{}", email)])
            .create_if_not_exists()
            .await?
    } else {
        directory
            .account()
            .terms_of_service_agreed(true)
            .contacts(vec![format!("mailto:{}", email)])
            .create_if_not_exists()
            .await?
    };
    Ok(account)
}

/// Resolve the solver to use based on user preference or auto-detection.
fn resolve_solver(
    solver_type: Option<SolverType>,
    dns_token: Option<String>,
) -> Result<(Box<dyn Solver + Send + Sync>, Option<tokio::task::JoinHandle<()>>)> {
    match solver_type {
        Some(SolverType::Http) => {
            check_port_80();
            let solver = Http01Solver::new();
            let handle = solver
                .start(&HTTP_LISTEN_ADDR.parse()?)
                .map_err(|e| anyhow::anyhow!("Failed to start HTTP-01 solver on {}: {}", HTTP_LISTEN_ADDR, e))?;
            info!("HTTP-01 solver listening on {}", HTTP_LISTEN_ADDR);
            // Spawn a task that waits for the handle to be stopped, then drops the solver
            let tokio_handle = tokio::task::spawn(async move {
                let _ = handle.stop().await;
            });
            Ok((Box::new(solver), Some(tokio_handle)))
        }
        Some(SolverType::Dns) => {
            let token = dns_token.context(
                "DNS-01 solver requires --dns-token or CLOUDFLARE_API_TOKEN environment variable"
            )?;
            let solver = CloudflareDns01Solver::new_with_token(token)
                .build()
                .map_err(|e| anyhow::anyhow!("Failed to build DNS-01 solver: {}", e))?;
            Ok((Box::new(solver), None))
        }
        None => {
            // Auto-detect: if no port 80 access (running without root), prefer DNS-01
            #[cfg(unix)]
            {
                let is_root = unsafe { libc::geteuid() } == 0;
                if is_root {
                    let solver = Http01Solver::new();
                    let handle = solver
                        .start(&HTTP_LISTEN_ADDR.parse()?)
                        .map_err(|e| anyhow::anyhow!("Failed to start HTTP-01 solver on {}: {}. Try --solver dns.", HTTP_LISTEN_ADDR, e))?;
                    info!("HTTP-01 solver listening on {}", HTTP_LISTEN_ADDR);
                    let tokio_handle = tokio::task::spawn(async move {
                        let _ = handle.stop().await;
                    });
                    Ok((Box::new(solver), Some(tokio_handle)))
                } else {
                    let token = dns_token.context(
                        "Not running as root. HTTP-01 challenge requires port 80. Use --solver dns with --dns-token, or run with sudo."
                    )?;
                    let solver = CloudflareDns01Solver::new_with_token(token)
                        .build()
                        .map_err(|e| anyhow::anyhow!("Failed to build DNS-01 solver: {}", e))?;
                    Ok((Box::new(solver), None))
                }
            }
            #[cfg(not(unix))]
            {
                let token = dns_token.context(
                    "DNS-01 solver requires --dns-token. HTTP-01 is not supported on this platform."
                )?;
                let solver = CloudflareDns01Solver::new_with_token(token)
                    .build()
                    .map_err(|e| anyhow::anyhow!("Failed to build DNS-01 solver: {}", e))?;
                Ok((Box::new(solver), None))
            }
        }
    }
}

async fn issue_cert(domain: &str, email: &str, staging: bool, solver_type: Option<SolverType>, dns_token: Option<String>) -> Result<()> {
    validate_domain(domain)?;

    let dir = domain_dir(domain);
    std::fs::create_dir_all(&dir)?;

    let url = get_directory_url(staging);
    let env = if staging { "STAGING" } else { "PRODUCTION" };
    info!("Connecting to Let's Encrypt {} ({})", env, url);

    let (solver, handle) = resolve_solver(solver_type, dns_token)?;

    let directory = Directory::builder(url)
        .http01_solver(Box::new(Http01Solver::new()))
        .dns01_solver(solver)
        .build()
        .await?;

    let account = create_or_load_account(&directory, email, domain).await?;

    // Save account key if newly created
    let akp = account_key_path(domain);
    if !akp.exists() {
        let pem = account
            .private_key()
            .private_key_to_pem_pkcs8()
            .map_err(|e| anyhow::anyhow!("Failed to serialize account key: {}", e))?;
        write_private(&akp, &pem)?;
        info!("Account key saved to {}", akp.display());
    }

    info!("Requesting certificate for {}...", domain);
    let certificate = account
        .certificate()
        .add_domain(domain)
        .obtain()
        .await?;

    let fullchain = certificate.fullchain_to_pem()?;
    let pk = certificate.private_key_to_pem()?;
    save_cert(domain, &fullchain, &pk)?;

    if let Some(handle) = handle {
        let _ = handle.await;
    }

    let cert_path_str = cert_path(domain).display().to_string();
    let key_path_str = key_path(domain).display().to_string();

    println!();
    println!("Certificate issued successfully!");
    println!("  Domain:     {}", domain);
    println!("  Cert:       {}", cert_path_str);
    println!("  Key:        {}", key_path_str);
    println!();
    println!("Use with llm-manager:");
    println!("  llm-manager serve --model MODEL.gguf --tls-cert {} --tls-key {}", cert_path_str, key_path_str);

    Ok(())
}

async fn renew_cert(domain: &str, email: &str, staging: bool, solver_type: Option<SolverType>, dns_token: Option<String>) -> Result<()> {
    validate_domain(domain)?;

    let cp = cert_path(domain);
    let kp = key_path(domain);

    if !cp.exists() || !kp.exists() {
        bail!(
            "No existing certificate found for '{}'. Run 'issue' first.\n  Cert: {}\n  Key: {}",
            domain,
            cp.display(),
            kp.display()
        );
    }

    let url = get_directory_url(staging);
    let env = if staging { "STAGING" } else { "PRODUCTION" };
    info!("Connecting to Let's Encrypt {} ({})", env, url);

    let chain = std::fs::read(&cp).context("Failed to read existing cert chain")?;
    let key = std::fs::read(&kp).context("Failed to read existing cert key")?;

    let existing_cert = lers::Certificate::from_chain_and_private_key(
        Format::Pem(chain.as_slice()),
        Format::Pem(key.as_slice()),
    )
    .context("Failed to parse existing certificate")?;

    let (solver, handle) = resolve_solver(solver_type, dns_token)?;

    let directory = Directory::builder(url)
        .http01_solver(Box::new(Http01Solver::new()))
        .dns01_solver(solver)
        .build()
        .await?;

    let account = create_or_load_account(&directory, email, domain).await?;

    info!("Renewing certificate for {}...", domain);
    let renewed = account.renew_certificate(existing_cert).await?;

    let fullchain = renewed.fullchain_to_pem()?;
    let pk = renewed.private_key_to_pem()?;
    save_cert(domain, &fullchain, &pk)?;

    if let Some(handle) = handle {
        let _ = handle.await;
    }

    let cert_path_str = cert_path(domain).display().to_string();
    let key_path_str = key_path(domain).display().to_string();

    println!();
    println!("Certificate renewed successfully!");
    println!("  Domain:     {}", domain);
    println!("  Cert:       {}", cert_path_str);
    println!("  Key:        {}", key_path_str);

    Ok(())
}

async fn revoke_cert(domain: &str, email: &str, staging: bool) -> Result<()> {
    validate_domain(domain)?;

    let cp = cert_path(domain);
    let kp = key_path(domain);

    if !cp.exists() || !kp.exists() {
        bail!(
            "No existing certificate found for '{}'.\n  Cert: {}\n  Key: {}",
            domain,
            cp.display(),
            kp.display()
        );
    }

    let url = get_directory_url(staging);
    info!("Connecting to Let's Encrypt ({})", url);

    let chain = std::fs::read(&cp).context("Failed to read existing cert chain")?;
    let key = std::fs::read(&kp).context("Failed to read existing cert key")?;

    let cert = lers::Certificate::from_chain_and_private_key(
        Format::Pem(chain.as_slice()),
        Format::Pem(key.as_slice()),
    )
    .context("Failed to parse existing certificate")?;

    let directory = Directory::builder(url)
        .build()
        .await?;

    let _account = create_or_load_account(&directory, email, domain).await?;

    info!("Revoking certificate for {}...", domain);
    cert.revoke(&directory).await?;

    // Remove local cert files
    let _ = std::fs::remove_file(&cp);
    let _ = std::fs::remove_file(&kp);

    println!();
    println!("Certificate revoked for: {}", domain);

    Ok(())
}

fn list_certs() -> Result<()> {
    let le_dir = letsencrypt_dir();

    if !le_dir.exists() {
        println!("No certificates found. Run 'llm-acme issue' to get one.");
        return Ok(());
    }

    println!("{:<25} {:<20} {:<15}", "DOMAIN", "EXPIRES", "DAYS_LEFT");
    println!("{:-<60}", "");

    let mut found = false;
    for entry in std::fs::read_dir(&le_dir)
        .context("Failed to read certificates directory")?
    {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        let cp = entry.path().join("cert.pem");

        if !cp.exists() {
            continue;
        }
        found = true;

        let content = std::fs::read(&cp)
            .with_context(|| format!("Failed to read {}", cp.display()))?;

        let (_, cert) = x509_parser::parse_x509_certificate(&content)
            .with_context(|| format!("Failed to parse cert at {}", cp.display()))?;

        let dt = cert.validity().not_after.to_datetime();
        let expires = chrono::DateTime::<chrono::Utc>::from_timestamp(
            dt.unix_timestamp(), dt.microsecond() as u32
        ).unwrap_or(chrono::Utc::now());

        let now = chrono::Utc::now();
        let remaining = expires.signed_duration_since(now);
        let days = remaining.num_days();

        let status = if days < 0 {
            "EXPIRED".to_string()
        } else if days < 30 {
            format!("{} days ⚠", days)
        } else {
            format!("{} days", days)
        };

        println!(
            "{:<25} {:<20} {:<15}",
            name,
            expires.format("%Y-%m-%d"),
            status
        );
    }

    if !found {
        println!("No certificates found. Run 'llm-acme issue' to get one.");
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("llm_acme=info".parse().unwrap()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Issue {
            domain,
            email,
            staging,
            solver,
            dns_token,
        } => {
            let email = get_email(email)?;
            issue_cert(&domain, &email, staging, solver, dns_token).await
        }
        Commands::Renew {
            domain,
            email,
            staging,
            solver,
            dns_token,
        } => {
            let email = get_email(email)?;
            renew_cert(&domain, &email, staging, solver, dns_token).await
        }
        Commands::Revoke {
            domain,
            email,
            staging,
        } => {
            let email = get_email(email)?;
            revoke_cert(&domain, &email, staging).await
        }
        Commands::List => list_certs(),
    }
}
