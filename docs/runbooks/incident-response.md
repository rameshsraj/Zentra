# Zentra Incident Response Runbook

## Service Health Check
1. Check API health: `curl http://10.88.11.162:5006/api/health`
2. Check process: `ps aux | grep zentra`
3. Check logs: `tail -100 /store/Zentra/logs/api.log`
4. Check database: `psql -h 10.88.11.162 -p 5434 -U postgres -d zentra -c "SELECT 1"`

## Common Issues

### API Not Responding
1. Check if process is running: `pgrep -f zentra-server`
2. Check port: `ss -tlnp | grep 5006`
3. Restart: `systemctl restart zentra-api`
4. Check logs for errors: `journalctl -u zentra-api -n 50`

### Database Connection Failed
1. Check PostgreSQL: `systemctl status postgresql`
2. Check connections: `psql -d zentra -c "SELECT count(*) FROM pg_stat_activity"`
3. Check pool exhaustion: Look for "pool timed out" in api.log

### High Memory Usage
1. Check process memory: `ps aux | grep zentra`
2. Check for flow aggregation growth (unbounded HashMap in netflow)
3. Restart if necessary: `systemctl restart zentra-api`

### Authentication Issues
1. Verify JWT secret is set: `grep ZENTRA_JWT_SECRET /store/Zentra/.env`
2. Check user lockout: `psql -d zentra -c "SELECT email, is_locked FROM users WHERE is_locked = true"`
3. Unlock user: `psql -d zentra -c "UPDATE users SET is_locked = false, locked_until = NULL WHERE email = '<email>'"`

## Escalation
1. Check this runbook first
2. Check docs/adr/ for architectural context
3. Search logs for error patterns
4. Escalate to on-call engineer
