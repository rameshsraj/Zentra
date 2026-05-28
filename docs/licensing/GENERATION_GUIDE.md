# Step-by-Step Guide: Generating a License Signature (`signature_hex`)

This document provides a complete, step-by-step technical walkthrough of how to generate the cryptographic signature (`signature_hex`) for a Zentra license file.

---

## The Workflow Overview

```
 ┌────────────────────────┐
 │ 1. Craft Payload JSON  │ ──> '{"org_id":"00000000-0000...","customer_name":"..."}'
 └────────────────────────┘
             │
             ▼
 ┌────────────────────────┐
 │ 2. Get UTF-8 Bytes     │ ──> [123, 34, 111, 114, 103, 95, 105, 100, ...]
 └────────────────────────┘
             │
             ▼
 ┌────────────────────────┐
 │ 3. Sign with PrivateKey│ ──> Ed25519_Sign(PrivateKey, PayloadBytes)
 └────────────────────────┘
             │
             ▼
 ┌────────────────────────┐
 │ 4. Hex-Encode Output   │ ──> "8e3fca40120150937a0...9fa" (64 bytes / 128 hex chars)
 └────────────────────────┘
```

---

## Step 1: Craft the Raw Payload JSON
The payload **must** be written as a valid JSON string containing the customer's entitlements, limits, and expiration metadata.

Create a compact JSON block with **no pretty-printed whitespace or line breaks** between properties. This is called the *canonical form* and ensures zero formatting discrepancies:

```json
{"org_id":"00000000-0000-0000-0000-000000000001","customer_name":"Short Term Evaluation Corp","onboarded_at":"2026-05-28T21:00:00Z","trial_duration_days":7,"max_targets":50,"features":{"enable_oracle":true,"enable_ebpf_network":true,"enable_ai_observability":true,"enable_otel":true,"allowed_menu_items":["dashboard"]}}
```

---

## Step 2: Convert to UTF-8 Raw Bytes
Convert the JSON string directly into its raw UTF-8 byte array equivalent. 

For example, the character `{` becomes byte `123` (`0x7B`), `"` becomes `34` (`0x22`), and so forth.

---

## Step 3: Compute the Ed25519 Asymmetric Signature
Using your secure offline **Ed25519 Private Key** (32-bytes raw seed or 64-bytes PKCS#8), sign the raw payload bytes computed in Step 2.

### Example in Python (Offline Admin Script)
```python
import json
import ed25519  # pip install ed25519

# 1. Load private key
# (Assume private_key_hex was generated securely)
private_key_hex = "4a1239bf...f03b" 
priv_key = ed25519.SigningKey(bytes.fromhex(private_key_hex))

# 2. Compact payload json string (Strictly no extra spacing)
payload_json = '{"org_id":"00000000-0000-0000-0000-000000000001","customer_name":"Short Term Evaluation Corp","onboarded_at":"2026-05-28T21:00:00Z","trial_duration_days":7,"max_targets":50,"features":{"enable_oracle":true,"enable_ebpf_network":true,"enable_ai_observability":true,"enable_otel":true,"allowed_menu_items":["dashboard"]}}'

# 3. Convert to UTF-8 bytes
payload_bytes = payload_json.encode('utf-8')

# 4. Sign and obtain the 64-byte signature
signature = priv_key.sign(payload_bytes)

# 5. Hex-encode to get the final signature_hex string (128 characters)
signature_hex = signature.hex()
print("SIGNATURE HEX:", signature_hex)
```

### Example in Node.js / JavaScript (Offline Portal Server)
```javascript
const crypto = require('crypto');

// 1. Your Ed25519 private key in PEM format
const privateKeyPem = `-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VwBCIEIP...
-----END PRIVATE KEY-----`;

// 2. Exact compact payload JSON string
const payloadJson = '{"org_id":"00000000-0000-0000-0000-000000000001","customer_name":"Short Term Evaluation Corp","onboarded_at":"2026-05-28T21:00:00Z","trial_duration_days":7,"max_targets":50,"features":{"enable_oracle":true,"enable_ebpf_network":true,"enable_ai_observability":true,"enable_otel":true,"allowed_menu_items":["dashboard"]}}';

const payloadBytes = Buffer.from(payloadJson, 'utf-8');

// 3. Compute Ed25519 Signature
const signature = crypto.sign(null, payloadBytes, privateKeyPem);

// 4. Output hex signature
console.log("SIGNATURE HEX:", signature.toString('hex'));
```

---

## Step 4: Construct the Sealed `license.json` File
Incorporate the raw `payload` object alongside the computed `signature_hex` inside the versioned envelope:

```json
{
  "format_version": 1,
  "algorithm": "ed25519",
  "key_id": "zentra-prod-2026-01",
  "payload": {
    "org_id": "00000000-0000-0000-0000-000000000001",
    "customer_name": "Short Term Evaluation Corp",
    "onboarded_at": "2026-05-28T21:00:00Z",
    "trial_duration_days": 7,
    "max_targets": 50,
    "features": {
      "enable_oracle": true,
      "enable_ebpf_network": true,
      "enable_ai_observability": true,
      "enable_otel": true,
      "allowed_menu_items": ["dashboard"]
    }
  },
  "signature_hex": "4f9dce38a168bc430e774c3e80f830da423cf95ee36b281cf6907ee9fb01725b80e4b85c160e1d0cb65089ef03fa0e224e7561a3cb90e0b3c662de341071da05"
}
```

Write this file to `/etc/zentra/license.json` on the target machine, and Zentra will execute its validation logic flawlessly.
