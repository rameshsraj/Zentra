#!/bin/bash
set -euo pipefail

POSTGRES_PASSWORD="${POSTGRES_PASSWORD:-zantaz}"
DEPLOY_PATH="/store/Zentra"
FAILED=0

echo "Running migrations from 20250101000059 onwards..."
shopt -s nullglob
mapfile -t all_files < <(find "${DEPLOY_PATH}/migrations" -maxdepth 1 -name '*.sql' -type f | sort)

for m in "${all_files[@]}"; do
  fname="$(basename "${m}")"
  # Skip migrations before 59 (already applied)
  num="${fname:0:14}"
  if [[ "${num}" < "20250101000059" ]]; then
    continue
  fi
  echo "  Applying: ${fname}"
  if ! PGPASSWORD="${POSTGRES_PASSWORD}" psql -v ON_ERROR_STOP=1 -h 127.0.0.1 -p 5434 -U postgres -d zentra -f "${m}" 2>&1; then
    echo "  FAILED: ${fname}" >&2
    FAILED=1
    break
  fi
done

echo "--- SUMMARY ---"
if [[ $FAILED -eq 0 ]]; then
  echo "All migrations completed successfully"
else
  echo "Migration failed - see error above"
fi
PGPASSWORD="${POSTGRES_PASSWORD}" psql -h 127.0.0.1 -p 5434 -U postgres -d zentra -tAc \
  "SELECT count(*) FROM information_schema.tables WHERE table_schema='public';" \
  | awk '{print "Public tables:", $1}'
