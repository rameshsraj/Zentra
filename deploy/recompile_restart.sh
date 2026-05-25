#!/bin/bash
set -e
cd /store/Zentra
source ~/.cargo/env 2>/dev/null

echo "=== Compiling ==="
cargo build --release --bin zentra-server --bin zentra-agent 2>&1 | tail -5

echo ""
echo "=== Restarting server ==="
pkill -f zentra-server || true
sleep 2
set -a; source .env 2>/dev/null; set +a
nohup ./target/release/zentra-server > /var/log/zentra-server.log 2>&1 &
echo "Server PID: $!"
sleep 3
curl -sf http://127.0.0.1:5006/api/health && echo "" || echo "Health check failed"
