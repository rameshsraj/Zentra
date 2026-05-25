//! Vault Integration for Oracle Credentials
//!
//! Fetches database credentials from external secret stores:
//! - OCI Vault (Oracle Cloud Infrastructure)
//! - Azure Key Vault
//! - HashiCorp Vault (KVv1, KVv2, Database mount)
//! - Password file (container-friendly)
//!
//! On ORA-01017 (invalid credentials), the connection pool can re-fetch
//! credentials and retry automatically.

use serde::{Deserialize, Serialize};
use tracing::{info, warn};

// ═══════════════════════════════════════════════════════════════════════════
// Configuration
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VaultProvider {
    None,
    OciVault { vault_id: String, secret_name: String, region: Option<String> },
    AzureKeyVault { vault_url: String, secret_name: String },
    HashiCorpVault(HashiCorpVaultConfig),
    PasswordFile { path: String },
}

impl Default for VaultProvider {
    fn default() -> Self { Self::None }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashiCorpVaultConfig {
    pub address: String,
    pub mount: String,           // "secret", "database", etc.
    pub path: String,            // "data/oracle/prod"
    pub key: String,             // field name within the secret
    pub auth: VaultAuth,
    pub namespace: Option<String>,
    pub tls_skip_verify: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VaultAuth {
    Token(String),
    AppRole { role_id: String, secret_id: String },
    Kubernetes { role: String, jwt_path: Option<String> },
}

impl Default for VaultAuth {
    fn default() -> Self { Self::Token(String::new()) }
}

// ═══════════════════════════════════════════════════════════════════════════
// Credential fetching
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultCredential {
    pub username: Option<String>,
    pub password: String,
    pub fetched_at: String,
    pub source: String,
}

/// Fetch credentials from the configured vault provider.
pub async fn fetch_credential(provider: &VaultProvider) -> Result<VaultCredential, String> {
    match provider {
        VaultProvider::None => Err("No vault provider configured".into()),
        VaultProvider::OciVault { vault_id, secret_name, region } => {
            get_oci_vault_secret(vault_id, secret_name, region.as_deref()).await
        }
        VaultProvider::AzureKeyVault { vault_url, secret_name } => {
            get_azure_vault_secret(vault_url, secret_name).await
        }
        VaultProvider::HashiCorpVault(config) => {
            get_hashicorp_vault_secret(config).await
        }
        VaultProvider::PasswordFile { path } => {
            read_password_file(path)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// OCI Vault
// ═══════════════════════════════════════════════════════════════════════════

async fn get_oci_vault_secret(
    vault_id: &str,
    secret_name: &str,
    _region: Option<&str>,
) -> Result<VaultCredential, String> {
    // OCI Vault REST API call
    // In production: use OCI SDK or HTTP call with instance principal auth
    // For now: structured placeholder that returns error if not configured
    info!(vault = %vault_id, secret = %secret_name, "Fetching OCI Vault secret");

    // TODO: implement real OCI Vault HTTP call
    // GET https://vaults.{region}.oci.oraclecloud.com/20180608/secrets/{secretId}
    Err(format!(
        "OCI Vault not yet configured. vault_id={}, secret={}. \
         Set OCI_CONFIG_FILE or use instance principal auth.",
        vault_id, secret_name
    ))
}

// ═══════════════════════════════════════════════════════════════════════════
// Azure Key Vault
// ═══════════════════════════════════════════════════════════════════════════

async fn get_azure_vault_secret(
    vault_url: &str,
    secret_name: &str,
) -> Result<VaultCredential, String> {
    info!(vault = %vault_url, secret = %secret_name, "Fetching Azure Key Vault secret");

    // TODO: implement real Azure Key Vault HTTP call
    // GET {vaultUrl}/secrets/{secretName}?api-version=7.4
    // Auth: managed identity token from IMDS
    Err(format!(
        "Azure Key Vault not yet configured. url={}, secret={}. \
         Set AZURE_CLIENT_ID or use managed identity.",
        vault_url, secret_name
    ))
}

// ═══════════════════════════════════════════════════════════════════════════
// HashiCorp Vault
// ═══════════════════════════════════════════════════════════════════════════

async fn get_hashicorp_vault_secret(
    config: &HashiCorpVaultConfig,
) -> Result<VaultCredential, String> {
    info!(addr = %config.address, path = %config.path, "Fetching HashiCorp Vault secret");

    // Determine auth token
    let token = match &config.auth {
        VaultAuth::Token(t) if !t.is_empty() => t.clone(),
        VaultAuth::Token(_) => {
            // Try VAULT_TOKEN env var
            std::env::var("VAULT_TOKEN").map_err(|_| "No VAULT_TOKEN set".to_string())?
        }
        VaultAuth::AppRole { role_id, secret_id } => {
            // TODO: POST /v1/auth/approle/login
            warn!("AppRole auth not yet implemented. role_id={}", role_id);
            let _ = secret_id;
            return Err("AppRole auth not yet implemented".into());
        }
        VaultAuth::Kubernetes { role, jwt_path } => {
            // TODO: read JWT from service account, POST /v1/auth/kubernetes/login
            warn!("Kubernetes auth not yet implemented. role={}", role);
            let _ = jwt_path;
            return Err("Kubernetes auth not yet implemented".into());
        }
    };

    // Build URL based on mount type
    let url = if config.mount == "database" {
        format!("{}/v1/{}/creds/{}", config.address, config.mount, config.path)
    } else {
        // KV v2
        format!("{}/v1/{}/data/{}", config.address, config.mount, config.path)
    };

    // TODO: actual HTTP call
    // For now, log the attempt
    info!(url = %url, key = %config.key, "Would fetch from HashiCorp Vault (token=***{})", &token[token.len().saturating_sub(4)..]);

    Err(format!(
        "HashiCorp Vault HTTP client not yet wired. url={}. \
         Add reqwest or ureq dependency to enable.",
        url
    ))
}

// ═══════════════════════════════════════════════════════════════════════════
// Password File (container-friendly)
// ═══════════════════════════════════════════════════════════════════════════

fn read_password_file(path: &str) -> Result<VaultCredential, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read password file {}: {}", path, e))?;

    let password = content.lines().next().unwrap_or("").trim().to_string();
    if password.is_empty() {
        return Err(format!("Password file {} is empty", path));
    }

    info!(path = %path, "Read password from file");
    Ok(VaultCredential {
        username: None,
        password,
        fetched_at: chrono::Utc::now().to_rfc3339(),
        source: format!("file:{}", path),
    })
}

// ═══════════════════════════════════════════════════════════════════════════
// Credential rotation helper
// ═══════════════════════════════════════════════════════════════════════════

/// Check if an Oracle error indicates invalid credentials (ORA-01017).
pub fn is_auth_failure(error_code: i32) -> bool {
    error_code == 1017 || error_code == 28000 || error_code == 28001
}

/// Attempt to re-fetch credentials from vault and return new password.
pub async fn rotate_credentials(provider: &VaultProvider) -> Result<String, String> {
    match fetch_credential(provider).await {
        Ok(cred) => {
            info!(source = %cred.source, "Credentials rotated successfully");
            Ok(cred.password)
        }
        Err(e) => {
            warn!("Credential rotation failed: {}", e);
            Err(e)
        }
    }
}
