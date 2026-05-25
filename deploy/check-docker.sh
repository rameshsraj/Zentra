#!/bin/bash
# Run on 10.88.11.162 — hops to Oracle host and checks Docker
ssh -o StrictHostKeyChecking=accept-new -o ConnectTimeout=10 root@10.88.218.59 bash -s <<'REMOTE'
echo "=== Docker containers ==="
docker ps -a --format "table {{.Names}}\t{{.Status}}\t{{.Ports}}" 2>&1
echo ""
echo "=== Docker inspect ports ==="
for ctr in $(docker ps -aq); do
  name=$(docker inspect --format '{{.Name}}' "$ctr" 2>/dev/null)
  ports=$(docker inspect --format '{{json .HostConfig.PortBindings}}' "$ctr" 2>/dev/null)
  echo "$name -> $ports"
done
echo ""
echo "=== Listening ports ==="
ss -tlnp | grep -E '1521|5500|31521' || echo "No Oracle ports found"
echo ""
echo "=== Is it a pod (podman/k8s)? ==="
which podman 2>/dev/null && podman ps -a 2>&1 || echo "No podman"
which kubectl 2>/dev/null && kubectl get pods -A 2>&1 || echo "No kubectl"
which crictl 2>/dev/null && crictl ps -a 2>&1 || echo "No crictl"
REMOTE
