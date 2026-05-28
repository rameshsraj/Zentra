# Zentra Licensing CI Build Matrix

Since this repository does not use an active automated CI workflow (like GitHub Actions or GitLab CI), the following verification build matrix must be executed manually or by any newly configured pipeline runner to prevent the `prod-license` code path from rotting.

## Verification Build & Test Matrix

Ensure that the licensing module and overall application build and pass all tests under both development (default features) and production (`prod-license` feature) configurations:

### 1. Development Build and Test (No Licensing Verification)
This ensures the codebase builds and tests cleanly with licensing features fully compiled out.
```bash
# Verify compilation
cargo check --workspace --all-targets --no-default-features

# Run workspace tests
cargo test --workspace --no-default-features
```

### 2. Production Build and Test (With `prod-license` Enabled)
This compiles and validates all cryptographic signatures and license enforcement code paths.
```bash
# Verify compilation with prod-license
cargo check --workspace --all-targets --features zentra-core/prod-license,zentra-api/prod-license

# Run workspace tests with prod-license
cargo test --workspace --features zentra-core/prod-license,zentra-api/prod-license
```
