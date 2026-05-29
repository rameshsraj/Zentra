//! Zentra License Generator — Offline Ed25519 license signing tool.
//!
//! Subcommands:
//!   keygen  — Generate an Ed25519 keypair
//!   sign    — Sign a license payload and emit a license.json
//!   verify  — Verify a license file against a public key

use chrono::{NaiveDate, Utc};
use clap::{Parser, Subcommand};
use ring::rand::SystemRandom;
use ring::signature::{self, Ed25519KeyPair, KeyPair};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

// ============================================================================
// CLI Definition
// ============================================================================

#[derive(Parser)]
#[command(name = "zentra-license-gen", about = "Zentra Ed25519 License Generator")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate an Ed25519 keypair
    Keygen {
        /// Output directory for key files
        #[arg(long, default_value = "keys")]
        output: PathBuf,

        /// Key ID label (e.g. "zentra-prod-2026-01")
        #[arg(long, default_value_t = default_key_id())]
        key_id: String,
    },

    /// Sign a license payload and emit license.json
    Sign {
        /// Path to PKCS#8 private key file (DER format)
        #[arg(long)]
        private_key: PathBuf,

        /// Path to license config JSON file
        #[arg(long)]
        config: PathBuf,

        /// Output license file path
        #[arg(long, default_value = "license.json")]
        output: PathBuf,

        /// Key ID to embed in the envelope
        #[arg(long, default_value_t = default_key_id())]
        key_id: String,
    },

    /// Verify a license file against a public key
    Verify {
        /// Path to hex-encoded public key file
        #[arg(long)]
        public_key: PathBuf,

        /// Path to license.json file
        #[arg(long)]
        license: PathBuf,
    },
}

fn default_key_id() -> String {
    let now = Utc::now();
    format!("zentra-prod-{}", now.format("%Y-%m"))
}

// ============================================================================
// License Payload (matches zentra-core LicensePayload)
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
struct LicenseConfig {
    org_name: String,
    org_id: String,
    product: String,
    tier: String,
    seat_count: u32,
    onboarded_at: String,
    trial_duration_days: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct LicenseEnvelope {
    format_version: u32,
    algorithm: String,
    key_id: String,
    payload: serde_json::value::RawValue,
    signature_hex: String,
}

// ============================================================================
// Subcommand Implementations
// ============================================================================

fn cmd_keygen(output: &PathBuf, key_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all(output)?;

    let rng = SystemRandom::new();
    let pkcs8_bytes = Ed25519KeyPair::generate_pkcs8(&rng)
        .map_err(|e| format!("Key generation failed: {:?}", e))?;

    let key_pair = Ed25519KeyPair::from_pkcs8(pkcs8_bytes.as_ref())
        .map_err(|e| format!("Failed to parse generated key: {:?}", e))?;

    let public_key_bytes = key_pair.public_key().as_ref();
    let public_key_hex = hex::encode(public_key_bytes);

    // Write private key (PKCS#8 DER)
    let private_path = output.join("private.der");
    fs::write(&private_path, pkcs8_bytes.as_ref())?;
    println!("Private key (PKCS#8 DER): {}", private_path.display());

    // Write public key (hex)
    let public_path = output.join("public.hex");
    fs::write(&public_path, &public_key_hex)?;
    println!("Public key (hex):         {}", public_path.display());

    // Write key_id
    let key_id_path = output.join("key_id.txt");
    fs::write(&key_id_path, key_id)?;
    println!("Key ID:                   {}", key_id);

    println!("\n=== Public Key Hex (embed in EMBEDDED_PUBLIC_KEY_HEX) ===");
    println!("{}", public_key_hex);

    println!("\nSecurity: Keep private.der offline and secured. Never commit to VCS.");

    Ok(())
}

fn cmd_sign(
    private_key_path: &PathBuf,
    config_path: &PathBuf,
    output_path: &PathBuf,
    key_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Read private key
    let pkcs8_bytes = fs::read(private_key_path)?;
    let key_pair = Ed25519KeyPair::from_pkcs8(&pkcs8_bytes)
        .map_err(|e| format!("Invalid private key: {:?}", e))?;

    // Read config
    let config_str = fs::read_to_string(config_path)?;
    let config: LicenseConfig = serde_json::from_str(&config_str)?;

    // Validate date format
    NaiveDate::parse_from_str(&config.onboarded_at, "%Y-%m-%d")
        .map_err(|e| format!("Invalid onboarded_at date (expected YYYY-MM-DD): {}", e))?;

    // Build payload as compact JSON (canonical bytes)
    let payload_json = serde_json::to_string(&config)?;
    let payload_bytes = payload_json.as_bytes();

    // Sign the raw payload bytes
    let signature = key_pair.sign(payload_bytes);
    let signature_hex = hex::encode(signature.as_ref());

    // Build versioned envelope
    let raw_payload: Box<serde_json::value::RawValue> =
        serde_json::value::RawValue::from_string(payload_json.clone())?;

    let envelope = LicenseEnvelope {
        format_version: 1,
        algorithm: "ed25519".to_string(),
        key_id: key_id.to_string(),
        payload: raw_payload,
        signature_hex,
    };

    // Write license file
    let envelope_json = serde_json::to_string_pretty(&envelope)?;
    fs::write(output_path, &envelope_json)?;

    println!("License signed successfully!");
    println!("  Output:    {}", output_path.display());
    println!("  Org:       {} ({})", config.org_name, config.org_id);
    println!("  Product:   {} / {}", config.product, config.tier);
    println!("  Seats:     {}", config.seat_count);
    println!("  Onboarded: {}", config.onboarded_at);
    println!("  Trial:     {} days", config.trial_duration_days);
    println!("  Key ID:    {}", key_id);
    println!("  Algorithm: ed25519");
    println!("  Sig:       {}...{}", &envelope.signature_hex[..16], &envelope.signature_hex[envelope.signature_hex.len()-16..]);

    Ok(())
}

fn cmd_verify(
    public_key_path: &PathBuf,
    license_path: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    // Read public key hex
    let pub_hex = fs::read_to_string(public_key_path)?.trim().to_string();
    let pub_bytes = hex::decode(&pub_hex)?;

    // Read license file — we need the raw JSON to extract payload bytes
    let license_str = fs::read_to_string(license_path)?;
    let envelope: serde_json::Value = serde_json::from_str(&license_str)?;

    // Validate envelope fields
    let format_version = envelope.get("format_version")
        .and_then(|v| v.as_u64())
        .ok_or("Missing format_version")?;
    if format_version != 1 {
        return Err(format!("Unsupported format_version: {}", format_version).into());
    }

    let algorithm = envelope.get("algorithm")
        .and_then(|v| v.as_str())
        .ok_or("Missing algorithm")?;
    if algorithm != "ed25519" {
        return Err(format!("Unsupported algorithm: {}", algorithm).into());
    }

    let key_id = envelope.get("key_id")
        .and_then(|v| v.as_str())
        .ok_or("Missing key_id")?;

    let signature_hex = envelope.get("signature_hex")
        .and_then(|v| v.as_str())
        .ok_or("Missing signature_hex")?;
    let sig_bytes = hex::decode(signature_hex)?;

    // Extract payload as raw JSON string (exact bytes that were signed)
    // We need to find the "payload" value in the JSON and get its exact bytes
    let payload_value = envelope.get("payload")
        .ok_or("Missing payload")?;
    let payload_json = serde_json::to_string(payload_value)?;
    let payload_bytes = payload_json.as_bytes();

    // Verify signature
    let peer_key = signature::UnparsedPublicKey::new(&signature::ED25519, &pub_bytes);
    peer_key.verify(payload_bytes, &sig_bytes)
        .map_err(|_| "SIGNATURE VERIFICATION FAILED")?;

    println!("Signature verification: PASSED");
    println!("  Key ID:    {}", key_id);
    println!("  Algorithm: {}", algorithm);

    // Parse and display payload
    let config: LicenseConfig = serde_json::from_value(payload_value.clone())?;
    println!("  Org:       {} ({})", config.org_name, config.org_id);
    println!("  Product:   {} / {}", config.product, config.tier);
    println!("  Seats:     {}", config.seat_count);
    println!("  Onboarded: {}", config.onboarded_at);
    println!("  Trial:     {} days", config.trial_duration_days);

    // Check expiry
    let onboarded = NaiveDate::parse_from_str(&config.onboarded_at, "%Y-%m-%d")?;
    let expiry = onboarded + chrono::Duration::days(config.trial_duration_days);
    let today = Utc::now().date_naive();
    let days_remaining = (expiry - today).num_days();

    if days_remaining < 0 {
        println!("  Status:    EXPIRED ({} days ago)", -days_remaining);
    } else {
        println!("  Status:    VALID ({} days remaining, expires {})", days_remaining, expiry);
    }

    Ok(())
}

// ============================================================================
// Main
// ============================================================================

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Keygen { output, key_id } => cmd_keygen(&output, &key_id),
        Commands::Sign { private_key, config, output, key_id } => {
            cmd_sign(&private_key, &config, &output, &key_id)
        }
        Commands::Verify { public_key, license } => cmd_verify(&public_key, &license),
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
