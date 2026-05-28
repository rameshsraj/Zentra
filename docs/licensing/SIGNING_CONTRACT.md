# Zentra Licensing Architecture — Cryptographic Verification & Signing Contract

## 1. Asymmetric Security Model
To prevent reverse-engineering compromises, Zentra splits cryptographic licensing responsibilities into two strictly isolated halves:

```
┌──────────────────────────────────────┐       ┌─────────────────────────────────────┐
│   Offline Licensing Portal / Vault   │       │     Zentra Production Binary        │
│    (Holds Asymmetric Private Key)    │       │     (Holds Asymmetric Public Key)   │
├──────────────────────────────────────┤       ├─────────────────────────────────────┤
│  • Generates customers' payload      │       │  • Reads license.json from disk     │
│  • Serializes payload into raw JSON  │ ────> │  • Locates the exact payload block  │
│  • Signs payload raw bytes           │       │  • Validates signature with PubKey  │
│  • Emits signed license.json envelope│       │  • Rejects if modified by even 1bit │
└──────────────────────────────────────┘       └─────────────────────────────────────┘
```

* **The Public Key is public**: Embedded as a compile-time constant string (`EMBEDDED_PUBLIC_KEY_HEX`) in `@/crates/zentra-core/src/crypto/license_verifier.rs`. Even if extracted by reverse-engineering, a malicious user cannot generate valid signatures.
* **The Private Key is private**: Never committed to this repository, stored exclusively in an isolated generator environment or key vault.

---

## 2. The Verification & Signing Contract

To ensure 100% deterministic signature verification across different language platforms (e.g., Python portals, Go CLI tools, or Rust runtimes), Zentra avoids re-serializing parsed objects. Re-serialization is vulnerable to formatting drift (whitespace adjustments, alphabetically sorted keys, or floating-point formatting).

### The Byte-Level Guarantee
1. **The Signer** serializes the license payload struct to a raw JSON string. It signs the **exact, raw UTF-8 bytes** of this payload substring.
2. **The Verifier** uses `Box<RawValue>` to extract the exact payload substring as it exists on disk, verifying the signature against those precise bytes.

---

## 3. Key Generation & Operator Guidelines

### Option A: Using OpenSSL 3.x (Recommended)
Operators can generate standard Ed25519 keys via standard terminal utilities:

```bash
# 1. Generate the private key PEM file
openssl genpkey -algorithm ed25519 -out private.pem

# 2. Extract the raw 32-byte hex-encoded public key for EMBEDDED_PUBLIC_KEY_HEX
openssl pkey -in private.pem -pubout -outform DER | tail -c 32 | xxd -p -c 32
```

### Option B: Using Ring (Rust CLI/Script)
```rust
use ring::signature::{Ed25519KeyPair, KeyPair};
use ring::rand::SystemRandom;

fn main() {
    let rng = SystemRandom::new();
    let pkcs8_bytes = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
    let key_pair = Ed25519KeyPair::from_pkcs8(pkcs8_bytes.as_ref()).unwrap();
    
    println!("PRIVATE KEY (PKCS#8 Hex): {}", hex::encode(pkcs8_bytes.as_ref()));
    println!("PUBLIC KEY (32-byte Hex): {}", hex::encode(key_pair.public_key().as_ref()));
}
```

---

## 4. Format of the License File (`license.json`)

The final envelope output on disk is defined as follows:

```json
{
  "format_version": 1,
  "algorithm": "ed25519",
  "key_id": "zentra-prod-2026-01",
  "payload": {
    "org_id": "00000000-0000-0000-0000-000000000001",
    "customer_name": "Acme Corp",
    "onboarded_at": "2026-05-28T12:00:00Z",
    "trial_duration_days": 30,
    "max_targets": 500,
    "features": {
      "enable_oracle": true,
      "enable_ebpf_network": true,
      "enable_ai_observability": true,
      "enable_otel": true,
      "allowed_menu_items": ["dashboard"]
    }
  },
  "signature_hex": "8e3fca40120150937a0...9fa"
}
```
