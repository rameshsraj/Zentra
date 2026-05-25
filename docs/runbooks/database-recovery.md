# Zentra Database Recovery Runbook

## Database: zentra (PostgreSQL 15 + TimescaleDB)
## Host: 10.88.11.162:5434

## Regular Backup
```
./deploy/backup-db.sh --full
```

## Restore from Backup
```
# Stop API server
systemctl stop zentra-api

# Restore from dump
pg_restore -h 10.88.11.162 -p 5434 -U postgres -d zentra \
  --clean --if-exists \
  /store/Zentra/backups/zentra_full_YYYYMMDD_HHMMSS.dump

# Run any pending migrations
sqlx migrate run --database-url "postgresql://postgres:$PGPASSWORD@10.88.11.162:5434/zentra" --source ./migrations

# Restart API server
systemctl start zentra-api

# Verify
curl http://localhost:5006/api/health
```

## Point-in-Time Recovery
Requires WAL archiving to be enabled (not currently configured).
TODO: Configure continuous_archiving in postgresql.conf.

## Schema-Only Recovery
```
./deploy/backup-db.sh --schema-only
# Then restore schema and re-run migrations
```

## Important Notes
- All operations target the `zentra` database ONLY
- Do NOT restore to the `postgres` database
- Always backup before migration deployments
