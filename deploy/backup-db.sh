#!/usr/bin/env bash
# Zentra Database Backup Script
# Usage: ./deploy/backup-db.sh [--full | --schema-only]
# Backs up the zentra database only.
set -euo pipefail

# Configuration from environment (no hardcoded credentials)
DB_HOST="${POSTGRES_HOST:?Error: POSTGRES_HOST not set}"
DB_PORT="${POSTGRES_PORT:-5434}"
DB_USER="${POSTGRES_USER:?Error: POSTGRES_USER not set}"
DB_NAME="zentra"  # Always backup zentra database
BACKUP_DIR="${BACKUP_DIR:-/store/Zentra/backups}"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

# Require PGPASSWORD
: "${PGPASSWORD:?Error: PGPASSWORD not set}"

mkdir -p "${BACKUP_DIR}"

MODE="${1:---full}"

case "${MODE}" in
    --full)
        echo "Full backup of zentra database..."
        pg_dump -h "${DB_HOST}" -p "${DB_PORT}" -U "${DB_USER}" \
            -d "${DB_NAME}" -Fc --verbose \
            -f "${BACKUP_DIR}/zentra_full_${TIMESTAMP}.dump"
        echo "Backup saved: ${BACKUP_DIR}/zentra_full_${TIMESTAMP}.dump"
        ;;
    --schema-only)
        echo "Schema-only backup of zentra database..."
        pg_dump -h "${DB_HOST}" -p "${DB_PORT}" -U "${DB_USER}" \
            -d "${DB_NAME}" --schema-only \
            -f "${BACKUP_DIR}/zentra_schema_${TIMESTAMP}.sql"
        echo "Backup saved: ${BACKUP_DIR}/zentra_schema_${TIMESTAMP}.sql"
        ;;
    *)
        echo "Usage: $0 [--full | --schema-only]"
        exit 1
        ;;
esac

# Cleanup backups older than 30 days
find "${BACKUP_DIR}" -name "zentra_*.dump" -mtime +30 -delete 2>/dev/null || true
find "${BACKUP_DIR}" -name "zentra_*.sql" -mtime +30 -delete 2>/dev/null || true

echo "Backup complete. Files in ${BACKUP_DIR}:"
ls -lh "${BACKUP_DIR}"/zentra_*_${TIMESTAMP}.* 2>/dev/null || echo "(none found)"
