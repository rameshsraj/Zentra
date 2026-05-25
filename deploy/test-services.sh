#!/bin/bash
echo "=== Health Check (port 7001) ==="
curl -s http://127.0.0.1:7001/api/health
echo ""

echo "=== Health Check (port 6006) ==="
curl -s http://127.0.0.1:6006/api/health
echo ""

echo "=== MCP Tools (port 7001) ==="
curl -s http://127.0.0.1:7001/api/mcp/tools | head -c 500
echo ""

echo "=== MCP Chat Test (port 7001) ==="
curl -s -X POST http://127.0.0.1:7001/api/mcp/chat \
  -H "Content-Type: application/json" \
  -d '{"message":"Show Oracle tablespace usage","session_id":"test-1","mode":"mcp"}' | head -c 500
echo ""

echo "=== Frontend (port 6010) ==="
curl -s -o /dev/null -w "HTTP %{http_code}" http://127.0.0.1:6010/
echo ""

echo "=== Process List ==="
ps aux | grep Zentra | grep -v grep

echo "=== Listening Ports ==="
ss -tlnp | grep -E '6006|6010|7001'

echo "=== TESTS COMPLETE ==="
