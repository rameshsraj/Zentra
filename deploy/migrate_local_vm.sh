#!/usr/bin/env bash
set -euo pipefail

APP_USER="${APP_USER:-zentra}"
POSTGRES_PASSWORD="${POSTGRES_PASSWORD:-zantaz}"
DEPLOY_PATH="/store/Zentra"
PGDATA_PATH="/store/data"
PG_VERSION="16"

export DEBIAN_FRONTEND=noninteractive

wait_for_apt_locks() {
  local tries=120
  local delay=5
  while (( tries > 0 )); do
    if fuser /var/lib/dpkg/lock-frontend >/dev/null 2>&1 \
      || fuser /var/lib/dpkg/lock >/dev/null 2>&1 \
      || fuser /var/lib/apt/lists/lock >/dev/null 2>&1; then
      echo "apt/dpkg lock detected, waiting... (${tries} tries left)"
      sleep "${delay}"
      tries=$((tries - 1))
      continue
    fi
    return 0
  done
  echo "Timed out waiting for apt/dpkg lock release" >&2
  return 1
}

install_postgres_16() {
  if command -v psql >/dev/null 2>&1 && psql --version 2>/dev/null | grep -q "${PG_VERSION}"; then
    echo "PostgreSQL ${PG_VERSION} already installed"
    return 0
  fi

  wait_for_apt_locks
  apt-get update -y
  wait_for_apt_locks
  apt-get install -y ca-certificates curl gnupg lsb-release

  if ! apt-cache show postgresql-${PG_VERSION} >/dev/null 2>&1; then
    install -d /etc/apt/keyrings
    curl -fsSL https://www.postgresql.org/media/keys/ACCC4CF8.asc \
      | gpg --dearmor -o /etc/apt/keyrings/postgresql.gpg

    local codename
    codename="$(lsb_release -cs)"
    echo "deb [signed-by=/etc/apt/keyrings/postgresql.gpg] http://apt.postgresql.org/pub/repos/apt ${codename}-pgdg main" \
      > /etc/apt/sources.list.d/pgdg.list

    wait_for_apt_locks
    if ! apt-get update -y; then
      echo "Falling back to noble-pgdg repository"
      echo "deb [signed-by=/etc/apt/keyrings/postgresql.gpg] http://apt.postgresql.org/pub/repos/apt noble-pgdg main" \
        > /etc/apt/sources.list.d/pgdg.list
      wait_for_apt_locks
      apt-get update -y
    fi
  fi

  wait_for_apt_locks
  apt-get install -y postgresql-common postgresql-${PG_VERSION} postgresql-client-${PG_VERSION} postgresql-contrib-${PG_VERSION}
}

echo "[1/8] Installing PostgreSQL ${PG_VERSION} and system dependencies"
install_postgres_16
wait_for_apt_locks
apt-get install -y \
  build-essential \
  pkg-config \
  libssl-dev \
  libpq-dev \
  python3 \
  python3-pip \
  nodejs \
  npm \
  git \
  make \
  rustc \
  cargo

echo "[2/8] Creating OS user ${APP_USER}"
if ! id -u "${APP_USER}" >/dev/null 2>&1; then
  useradd -m -s /bin/bash "${APP_USER}"
fi


echo "[3/8] Preparing /store and cluster data path at ${PGDATA_PATH}"
mkdir -p /store "${DEPLOY_PATH}" "${PGDATA_PATH}"
chown -R postgres:postgres "${PGDATA_PATH}"
chmod 700 "${PGDATA_PATH}"

echo "[4/8] Resetting PostgreSQL clusters and creating PostgreSQL ${PG_VERSION} cluster"
if command -v pg_lsclusters >/dev/null 2>&1; then
  while read -r version name port status owner data_dir log_file; do
    if [[ -n "${version}" && "${version}" != "Ver" ]]; then
      pg_dropcluster --stop "${version}" "${name}" || true
    fi
  done < <(pg_lsclusters || true)
fi

pg_createcluster --datadir "${PGDATA_PATH}" --start "${PG_VERSION}" main
PG_CONF_DIR="/etc/postgresql/${PG_VERSION}/main"
PG_CONF_FILE="${PG_CONF_DIR}/postgresql.conf"
PG_HBA_FILE="${PG_CONF_DIR}/pg_hba.conf"
sed -i -E "s/^#?[[:space:]]*port[[:space:]]*=[[:space:]]*[0-9]+/port = 5434/" "${PG_CONF_FILE}"
sed -i -E "s/^#?[[:space:]]*listen_addresses[[:space:]]*=.*/listen_addresses = '127.0.0.1'/" "${PG_CONF_FILE}"
if ! grep -q "127.0.0.1/32" "${PG_HBA_FILE}"; then
  printf "host    all             all             127.0.0.1/32            scram-sha-256\n" >> "${PG_HBA_FILE}"
fi
# Enable timescaledb if installed
if ls /usr/share/postgresql/${PG_VERSION}/extension/timescaledb.control >/dev/null 2>&1; then
  sed -i -E "s/^#?[[:space:]]*shared_preload_libraries[[:space:]]*=.*/shared_preload_libraries = 'timescaledb'/" "${PG_CONF_FILE}"
fi
systemctl enable postgresql
systemctl restart postgresql

echo "[5/8] Configuring postgres password and creating database zentra"
sudo -u postgres psql -p 5434 -v ON_ERROR_STOP=1 -c "ALTER USER postgres WITH PASSWORD '${POSTGRES_PASSWORD}';"
if ! sudo -u postgres psql -p 5434 -tAc "SELECT 1 FROM pg_database WHERE datname='zentra'" | grep -q 1; then
  sudo -u postgres createdb -p 5434 zentra
fi

echo "[6/8] Unpacking project into ${DEPLOY_PATH}"
rm -rf "${DEPLOY_PATH}"/*
mkdir -p "${DEPLOY_PATH}"
TAR_PATH=""
for candidate in /store/zentra-sync.tar.gz /tmp/zentra-sync.tar.gz; do
  if [[ -f "${candidate}" ]]; then
    TAR_PATH="${candidate}"
    break
  fi
done
if [[ -z "${TAR_PATH}" ]]; then
  echo "Missing zentra-sync.tar.gz in /store or /tmp" >&2
  exit 1
fi
tar -xzf "${TAR_PATH}" -C "${DEPLOY_PATH}"

echo "[7/8] Installing project dependencies"
if [[ -f "${DEPLOY_PATH}/frontend/package.json" ]]; then
  cd "${DEPLOY_PATH}/frontend"
  npm ci || npm install
fi
cd "${DEPLOY_PATH}"
if [[ -f Cargo.toml ]]; then
  cargo fetch
fi

echo "[8/8] Running all SQL migrations into database zentra"
if [[ -d "${DEPLOY_PATH}/migrations" ]]; then
  shopt -s nullglob
  mapfile -t migration_files < <(find "${DEPLOY_PATH}/migrations" -maxdepth 1 -name '*.sql' -type f | sort)
  for m in "${migration_files[@]}"; do
    echo "Applying migration: $(basename "${m}")"
    PGPASSWORD="${POSTGRES_PASSWORD}" psql -v ON_ERROR_STOP=1 -h 127.0.0.1 -p 5434 -U postgres -d zentra -f "${m}" >/dev/null
  done
fi

chown -R "${APP_USER}:${APP_USER}" "${DEPLOY_PATH}"

echo "--- SUMMARY ---"
echo "Host: $(hostname)"
echo "OS user: ${APP_USER}"
echo "PostgreSQL: $(psql --version 2>/dev/null || true)"
echo "PG data path: ${PGDATA_PATH}"
echo "Deploy path: ${DEPLOY_PATH}"
echo "Database: zentra"
PGPASSWORD="${POSTGRES_PASSWORD}" psql -h 127.0.0.1 -p 5434 -U postgres -d zentra -tAc "SELECT count(*) FROM information_schema.tables WHERE table_schema='public';" | awk '{print "Public tables:", $1}'
