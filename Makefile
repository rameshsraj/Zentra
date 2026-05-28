.PHONY: release-prod build test check dev-build dev-test

# Production release target with licensing enabled (T03)
release-prod:
	cargo build --release --features zentra-core/prod-license,zentra-api/prod-license -p zentra-api -p zentra-agent

# Standard build
build:
	cargo build --workspace

# Standard check
check:
	cargo check --workspace

# Standard test
test:
	cargo test --workspace

# Dev-only build (licensing compiled out)
dev-build:
	cargo build --workspace --no-default-features

# Dev-only test
dev-test:
	cargo test --workspace --no-default-features
