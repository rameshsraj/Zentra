#!/bin/bash
set -e

# Source port configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${SCRIPT_DIR}/ports.env" 2>/dev/null || true

echo "=== Stopping old services ==="
pkill -f zentra-server 2>/dev/null || true
pkill -f zentra-agent 2>/dev/null || true
fuser -k ${ZENTRA_API_PORT}/tcp 2>/dev/null || true
fuser -k ${ZENTRA_AGENT_PORT}/tcp 2>/dev/null || true
sleep 2

echo "=== Checking binary ==="
ls -la /store/Zentra/target/release/zentra-server
ls -la /store/Zentra/target/release/zentra-agent

echo "=== Creating log directory ==="
mkdir -p /store/Zentra/logs

echo "=== Starting zentra-server ==="
cd /store/Zentra
nohup ./target/release/zentra-server > /store/Zentra/logs/server.log 2>&1 &
SERVER_PID=$!
echo "Server PID: $SERVER_PID"

echo "=== Starting zentra-agent ==="
nohup ./target/release/zentra-agent > /store/Zentra/logs/agent.log 2>&1 &
AGENT_PID=$!
echo "Agent PID: $AGENT_PID"

echo "=== Waiting for startup ==="
sleep 5

echo "=== Server log ==="
cat /store/Zentra/logs/server.log 2>/dev/null | tail -30

echo "=== Health check ==="
curl -s http://127.0.0.1:${ZENTRA_API_PORT}/api/health 2>&1 || echo "API not responding"

echo "=== MCP tools check ==="
curl -s http://127.0.0.1:${ZENTRA_API_PORT}/api/mcp/tools 2>&1 | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    print(f'MCP Tools: {len(data)} tools loaded')
    for t in data[:5]:
        print(f'  - {t[\"name\"]} ({t[\"category\"]})')
    if len(data) > 5:
        print(f'  ... and {len(data)-5} more')
except:
    print('MCP endpoint not responding')
" 2>&1

echo "=== Process check ==="
ps aux | grep Zentra | grep -v grep

echo "=== RESTART COMPLETE ==="
