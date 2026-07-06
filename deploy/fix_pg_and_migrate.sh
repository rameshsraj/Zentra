#!/bin/bash
set -euo pipefail

POSTGRES_PASSWORD="${POSTGRES_PASSWORD:-zantaz}"
DEPLOY_PATH="/store/Zentra"
PG_CONF="/etc/postgresql/16/main/postgresql.conf"

echo "[1] Setting port to 5434"
sed -i -E 's/^#?[[:space:]]*port[[:space:]]*=[[:space:]]*[0-9]+/port = 5434/' "$PG_CONF"

echo "[2] Enabling timescaledb in shared_preload_libraries"
if grep -q '^#*shared_preload_libraries' "$PG_CONF"; then
  sed -i -E "s|^#?[[:space:]]*shared_preload_libraries[[:space:]]*=.*|shared_preload_libraries = 'timescaledb'|" "$PG_CONF"
else
  echo "shared_preload_libraries = 'timescaledb'" >> "$PG_CONF"
fi

echo "--- Config verification ---"
grep -E "^port|shared_preload" "$PG_CONF"

echo "[3] Restarting PostgreSQL"
systemctl restart postgresql
sleep 3
pg_lsclusters

echo "[4] Setting postgres password and ensuring zentra DB exists"
sudo -u postgres psql -p 5434 -v ON_ERROR_STOP=1 -c "ALTER USER postgres WITH PASSWORD '${POSTGRES_PASSWORD}';"
if ! sudo -u postgres psql -p 5434 -tAc "SELECT 1 FROM pg_database WHERE datname='zentra'" | grep -q 1; then
  sudo -u postgres createdb -p 5434 zentra
  echo "Database zentra created"
else
  echo "Database zentra already exists"
fi

echo "[5] Running SQL migrations"
shopt -s nullglob
mapfile -t migration_files < <(find "${DEPLOY_PATH}/migrations" -maxdepth 1 -name '*.sql' -type f | sort)
if [[ ${#migration_files[@]} -eq 0 ]]; then
  echo "No migration files found in ${DEPLOY_PATH}/migrations" >&2
  exit 1
fi
for m in "${migration_files[@]}"; do
  echo "  Applying: $(basename "${m}")"
  PGPASSWORD="${POSTGRES_PASSWORD}" psql -v ON_ERROR_STOP=1 -h 127.0.0.1 -p 5434 -U postgres -d zentra -f "${m}"
done

echo "--- DONE ---"
PGPASSWORD="${POSTGRES_PASSWORD}" psql -h 127.0.0.1 -p 5434 -U postgres -d zentra -tAc \
  "SELECT count(*) FROM information_schema.tables WHERE table_schema='public';" \
  | awk '{print "Public tables:", $1}'
