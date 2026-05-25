#!/bin/bash
# Run on 10.88.11.162 — hops to Oracle host and starts Docker
ssh -o StrictHostKeyChecking=accept-new -o ConnectTimeout=10 root@10.88.218.59 bash -s <<'REMOTE'
echo "=== hostname ==="
hostname
echo "=== docker ps -a ==="
docker ps -a 2>&1
echo "=== starting all stopped containers ==="
docker start $(docker ps -aq) 2>&1
echo "=== waiting 10s ==="
sleep 10
echo "=== docker ps ==="
docker ps 2>&1
echo "=== checking port 1521 ==="
ss -tlnp | grep 1521 || echo "port 1521 not listening yet"
echo "=== checking port 31521 ==="
ss -tlnp | grep 31521 || echo "port 31521 not listening yet"
echo "=== DONE ==="
REMOTE
