#!/bin/bash
set -e

REPO_DIR="/store/Zentra"
STAGE_DIR="/tmp/zentra-build-export"
WINDOWS_OUTPUT="/mnt/c/GitHub/Zentra/artifacts/vm-build"

echo "========== WSL Artifact Package Script =========="
echo ""

# Verify artifacts exist
echo "1. Verifying artifacts..."
if [ ! -f "$REPO_DIR/target/release/zentra-server" ]; then
    echo "ERROR: Backend binary not found at $REPO_DIR/target/release/zentra-server"
    exit 1
fi
BINARY_SIZE=$(ls -lh "$REPO_DIR/target/release/zentra-server" | awk '{print $5}')
echo "✓ Backend binary: $BINARY_SIZE"

if [ ! -d "$REPO_DIR/frontend/dist" ]; then
    echo "ERROR: Frontend dist not found at $REPO_DIR/frontend/dist"
    exit 1
fi
FRONTEND_FILES=$(find "$REPO_DIR/frontend/dist" -type f | wc -l)
echo "✓ Frontend files: $FRONTEND_FILES files"

if [ ! -d "$REPO_DIR/migrations" ]; then
    echo "ERROR: Migrations directory not found at $REPO_DIR/migrations"
    exit 1
fi
MIGRATION_COUNT=$(ls "$REPO_DIR/migrations"/*.sql 2>/dev/null | wc -l)
echo "✓ Migration files: $MIGRATION_COUNT files"

# Create staging directory
echo ""
echo "2. Creating staging directory..."
rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR/build/frontend-dist"
mkdir -p "$STAGE_DIR/migrations"
echo "✓ Staging directory created"

# Copy artifacts
echo ""
echo "3. Copying artifacts..."
cp "$REPO_DIR/target/release/zentra-server" "$STAGE_DIR/build/"
echo "✓ Backend binary copied"

cp -r "$REPO_DIR/frontend/dist/"* "$STAGE_DIR/build/frontend-dist/"
echo "✓ Frontend assets copied"

cp "$REPO_DIR/migrations"/*.sql "$STAGE_DIR/migrations/"
echo "✓ Migrations copied"

# Create .env.example
echo ""
echo "4. Creating .env.example..."
cat > "$STAGE_DIR/.env.example" << 'ENVEOF'
DATABASE_URL=postgresql://postgres:zentra@127.0.0.1:55434/zentra
ZENTRA_API_PORT=5006
ZENTRA_BIND_ADDR=0.0.0.0
ZENTRA_JWT_SECRET=change_me_min_32_chars
ZENTRA_MASTER_KEY=change_me_64_hex_chars
RUST_LOG=info
ENVEOF
echo "✓ .env.example created"

# Create tar.gz archive
echo ""
echo "5. Creating compressed archive..."
cd /tmp
tar -czf "zentra-export.tar.gz" "zentra-build-export/"
ARCHIVE_SIZE=$(ls -lh zentra-export.tar.gz | awk '{print $5}')
echo "✓ Archive created: $ARCHIVE_SIZE"

# Copy to Windows mount
echo ""
echo "6. Copying to Windows mount..."
mkdir -p "$WINDOWS_OUTPUT"
cp zentra-export.tar.gz "$WINDOWS_OUTPUT/"
echo "✓ Copied to $WINDOWS_OUTPUT/zentra-export.tar.gz"

# Extract on Windows side for easier access
echo ""
echo "7. Extracting archive for direct access..."
cd "$WINDOWS_OUTPUT"
tar -xzf zentra-export.tar.gz
if [ -d "zentra-build-export" ]; then
    mv zentra-build-export zentra-export
fi
echo "✓ Archive extracted to $WINDOWS_OUTPUT/zentra-export/"

# Verify
echo ""
echo "========== VERIFICATION =========="
echo "Output location: $WINDOWS_OUTPUT/"
ls -lah "$WINDOWS_OUTPUT/" | tail -20
echo ""
if [ -d "$WINDOWS_OUTPUT/zentra-export" ]; then
    echo "Archive contents:"
    ls -lah "$WINDOWS_OUTPUT/zentra-export/"
fi
echo ""
echo "========== SUCCESS =========="
echo "Artifacts ready for GCP deployment!"
echo "Binary: $WINDOWS_OUTPUT/zentra-export/build/zentra-server"
echo "Frontend: $WINDOWS_OUTPUT/zentra-export/build/frontend-dist/"
echo "Migrations: $WINDOWS_OUTPUT/zentra-export/migrations/"
