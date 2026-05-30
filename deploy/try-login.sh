#!/bin/bash
BASE=http://127.0.0.1:5006
echo "=== Trying different credentials ==="
echo "--- admin@zentra.local / admin ---"
curl -s -X POST $BASE/api/auth/login -H 'Content-Type: application/json' -d '{"email":"admin@zentra.local","password":"admin"}'
echo ""
echo "--- admin@zentra.local / skyline ---"
curl -s -X POST $BASE/api/auth/login -H 'Content-Type: application/json' -d '{"email":"admin@zentra.local","password":"skyline"}'
echo ""
echo "--- satheesh@zentra.local / admin ---"
curl -s -X POST $BASE/api/auth/login -H 'Content-Type: application/json' -d '{"email":"satheesh@zentra.local","password":"admin"}'
echo ""
echo "--- satheesh@zentra.local / skyline ---"
curl -s -X POST $BASE/api/auth/login -H 'Content-Type: application/json' -d '{"email":"satheesh@zentra.local","password":"skyline"}'
echo ""
echo "--- admin@zentra.local / Admin@123 ---"
curl -s -X POST $BASE/api/auth/login -H 'Content-Type: application/json' -d '{"email":"admin@zentra.local","password":"Admin@123"}'
echo ""
echo "--- admin@zentra.local / password ---"
curl -s -X POST $BASE/api/auth/login -H 'Content-Type: application/json' -d '{"email":"admin@zentra.local","password":"password"}'
echo ""
