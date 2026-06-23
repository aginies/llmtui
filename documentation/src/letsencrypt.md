# Let's Encrypt Certificates

The `llm-acme` tool is a dedicated CLI for obtaining, renewing, and managing Let's Encrypt TLS certificates. Certificates are stored in `~/.config/llm-manager/tls/letsencrypt/` and can be used with llm-manager's serve mode or API endpoint.

## Installation

`llm-acme` is built as part of the llm-manager workspace:

```bash
cargo build -p llm-acme --release
```

The binary is available at `target/release/llm-acme`.

## Commands

### Issue a Certificate

Obtain a new certificate for a domain:

```bash
sudo llm-acme issue --domain myhost.example.com --email user@example.com
```

The tool binds to port 80 to complete the HTTP-01 ACME challenge. Root privileges are required (or `CAP_NET_BIND_SERVICE` capability).

### Renew a Certificate

Renew an existing certificate:

```bash
sudo llm-acme renew --domain myhost.example.com --email user@example.com
```

### Revoke a Certificate

Revoke and remove a certificate:

```bash
sudo llm-acme revoke --domain myhost.example.com --email user@example.com
```

### List Certificates

View all stored certificates and their expiry dates:

```bash
llm-acme list
```

Output:
```
DOMAIN                      EXPIRES              DAYS_LEFT
------------------------------------------------------------
myhost.example.com          2026-09-15           84 days
```

Certificates expiring within 30 days are marked with a warning indicator.

## Options

| Option | Description |
|--------|-------------|
| `--domain`, `-d` | Domain name for the certificate |
| `--email` | Email address for Let's Encrypt account (falls back to config.yaml) |
| `--staging` | Use Let's Encrypt staging server for testing |

## Email Configuration

The email address can be provided via the `--email` flag or stored in `config.yaml`:

```yaml
letsencrypt_email: "user@example.com"
```

When set in config.yaml, the `--email` flag is optional.

## Staging Mode

Use `--staging` to test against Let's Encrypt's staging server. Staging certificates are not trusted by browsers but are useful for validating configuration before requesting production certificates.

```bash
sudo llm-acme issue --domain myhost.example.com --email test@example.com --staging
```

## Certificate Storage

Certificates are stored per-domain in:

```
~/.config/llm-manager/tls/letsencrypt/<domain>/
├── cert.pem           # Full certificate chain (public)
├── key.pem            # Private key (mode 0600)
└── account-key.pem    # ACME account key (mode 0600)
```

Private keys are written with restricted permissions (`0600`) on Unix systems.

## Using with llm-manager

### Serve Mode

Pass the cert and key paths to the serve command:

```bash
llm-manager serve --model model.gguf \
  --tls-cert ~/.config/llm-manager/tls/letsencrypt/myhost.example.com/cert.pem \
  --tls-key  ~/.config/llm-manager/tls/letsencrypt/myhost.example.com/key.pem
```

### Config File

Set the paths in `~/.config/llm-manager/config.yaml`:

```yaml
default:
  server_tls_enabled: true
  server_tls_cert: ~/.config/llm-manager/tls/letsencrypt/myhost.example.com/cert.pem
  server_tls_key: ~/.config/llm-manager/tls/letsencrypt/myhost.example.com/key.pem
```

### API Endpoint

The API proxy shares TLS configuration with the WebSocket dashboard. When enabled, all API requests use HTTPS:

```yaml
default:
  server_tls_enabled: true
  server_tls_cert: /path/to/cert.pem
  server_tls_key: /path/to/key.pem
  api_endpoint_enabled: true
  api_endpoint_port: 49222
```

Or from the command line:

```bash
llm-manager serve --model model.gguf \
  --tls-cert /path/to/cert.pem \
  --tls-key /path/to/key.pem
```

With TLS enabled, the API proxy listens on `https://` instead of `http://`. Clients must use the `https://` base URL and trust the CA that signed the certificate.

### Using llm-acme certs with the API

After issuing a certificate with `llm-acme`, configure it in `config.yaml`:

```yaml
default:
  server_tls_enabled: true
  server_tls_cert: ~/.config/llm-manager/tls/letsencrypt/myhost.example.com/cert.pem
  server_tls_key: ~/.config/llm-manager/tls/letsencrypt/myhost.example.com/key.pem
  api_endpoint_enabled: true
  api_endpoint_port: 49222
```

The API proxy at `https://myhost.example.com:49222` will present the Let's Encrypt certificate, trusted by all standard clients.

### Using your own certificates

You can use any valid TLS certificate (self-signed, CA-signed, etc.):

```bash
llm-manager serve --model model.gguf \
  --tls-cert /etc/ssl/certs/my-cert.pem \
  --tls-key /etc/ssl/private/my-key.pem
```

Place the paths in `config.yaml` under `server_tls_cert` and `server_tls_key`. The cert and key files must exist; if they do not, llm-manager auto-generates a self-signed certificate instead.

## Port 80 Requirement

The HTTP-01 ACME challenge requires binding to port 80. You have several options:

### Run with sudo

```bash
sudo llm-acme issue --domain myhost.example.com --email user@example.com
```

### Set capability

Grant the binary the `CAP_NET_BIND_SERVICE` capability to bind to port 80 without full root:

```bash
sudo setcap cap_net_bind_service=+ep target/release/llm-acme
```

Then run normally:

```bash
llm-acme issue --domain myhost.example.com --email user@example.com
```

### Use authbind

Configure `authbind` to allow specific users to bind to privileged ports:

```bash
sudo apt install authbind
authbind --deep llm-acme issue --domain myhost.example.com --email user@example.com
```

## Automatic Renewal

Certificates issued by Let's Encrypt expire after 90 days. Set up a cron job to renew automatically:

```cron
# Renew certificates monthly
0 3 1 * * sudo /path/to/llm-acme renew --domain myhost.example.com --email user@example.com
```

Or use a systemd timer for more flexible scheduling.

## Domain Validation

`llm-acme` validates domain names before making ACME requests:

- Maximum 253 characters total
- Each label max 63 characters
- Only alphanumeric characters and hyphens allowed
- Labels cannot start or end with hyphens
- No empty labels (consecutive dots or trailing dots)

Invalid domains are rejected early with a clear error message.

## Cloudflare API Token

DNS-01 challenges require a Cloudflare API token. Create one:

1. Go to [Cloudflare API Tokens](https://dash.cloudflare.com/profile/api-tokens)
2. Click **Create Token**
3. Use the **Edit zone DNS** template
4. Under **Zone Resources**, select your domain (or All Zones)
5. Under **Permissions**, ensure **Zone → DNS → Edit** is included
6. Click **Continue to summary**, then **Create Token**
7. Copy the token — it will only be shown once

The token must have `Zone:Read` and `DNS:Edit` permissions.

### Using the token

Set it as an environment variable:

```bash
export CLOUDFLARE_API_TOKEN=<your-token>
sudo llm-acme issue --domain llm.guibo.com --email antoine@ginies.org
```

Or pass it directly:

```bash
sudo llm-acme issue --domain llm.guibo.com --email antoine@ginies.org --dns-token <your-token>
```

The ACME account key is persisted per-domain in `account-key.pem`. This allows certificate renewal without re-registering the account. The same account key is reused across issue and renew operations for the same domain.
