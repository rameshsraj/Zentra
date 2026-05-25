//! Vault Configuration Models (V5 Prompt 16)

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VaultProviderType {
    OciVault,
    AzureKeyVault,
    HashiCorpVault,
    PasswordFile,
    Environment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultConfig {
    pub provider: VaultProviderType,
    pub config: VaultProviderConfig,
    pub cache_ttl_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum VaultProviderConfig {
    OciVault {
        compartment_id: String,
        vault_id: String,
        secret_id: String,
        region: Option<String>,
    },
    AzureKeyVault {
        vault_url: String,
        secret_name: String,
        use_managed_identity: bool,
        tenant_id: Option<String>,
        client_id: Option<String>,
    },
    HashiCorpVault {
        address: String,
        auth_method: HashiCorpAuthMethod,
        mount_path: String,
        secret_path: String,
        key_name: Option<String>,
        namespace: Option<String>,
        tls_skip_verify: bool,
    },
    PasswordFile {
        path: String,
    },
    Environment {
        variable: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HashiCorpAuthMethod {
    Token { token: String },
    AppRole { role_id: String, secret_id: String },
    Kubernetes { role: String, jwt_path: Option<String> },
    UnixSocket { path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultCredentials {
    pub username: String,
    pub password: String,
    pub fetched_at: String,
    pub expires_at: Option<String>,
}
