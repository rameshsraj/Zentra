# Zentra Production Security Hardening Checklist

## Before First Production Deploy

- [ ] Set `ZENTRA_JWT_SECRET` to a random 64+ character string
- [ ] Set `ZENTRA_MASTER_KEY` to a random 64-character hex string (32 bytes)
- [ ] Set `ZENTRA_ALLOW_INSECURE_JWT=0` (or unset — default is secure)
- [ ] Change default admin password via `/api/auth/change-password`
- [ ] Set `ZENTRA_ALLOW_ORIGINS` to specific frontend URLs (not `*`)
- [ ] Configure PostgreSQL to require SSL connections
- [ ] Set up SSH key-based auth for all monitored targets
- [ ] Create non-root `zentra` service user
- [ ] Enable nginx with TLS (HTTPS)
- [ ] Configure firewall rules (only allow 443 from outside)
- [ ] Set `ZENTRA_SSH_KNOWN_HOSTS=/root/.ssh/known_hosts`
- [ ] Remove `deploy/login.json` and `deploy/oraconn.json` from server
- [ ] Rotate all passwords from development (PostgreSQL, Oracle)

## Database (zentra)
- [ ] PostgreSQL user for app should NOT be superuser
- [ ] Enable RLS policies (Phase 5)
- [ ] Configure automated backups (`deploy/backup-db.sh` via cron)
- [ ] Enable TimescaleDB retention policies
- [ ] Set `statement_timeout` in postgresql.conf

## Ongoing
- [ ] Review audit_log table monthly
- [ ] Rotate JWT secret quarterly
- [ ] Run `cargo audit` and `npm audit` weekly
- [ ] Review nginx access logs for suspicious patterns
