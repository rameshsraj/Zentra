#!/bin/bash
ssh -o StrictHostKeyChecking=accept-new -o ConnectTimeout=10 root@10.88.218.59 bash -s <<'REMOTE'
echo "=== K8s services in oracle namespace ==="
kubectl get svc -n oracle -o wide 2>&1
echo ""
echo "=== K8s pod details ==="
kubectl get pods -n oracle -o wide 2>&1
echo ""
echo "=== K8s service YAML ==="
kubectl get svc -n oracle -o yaml 2>&1
echo ""
echo "=== NodePort check ==="
kubectl get svc -n oracle -o jsonpath='{range .items[*]}{.metadata.name}: type={.spec.type}, ports={.spec.ports[*].nodePort}{"\n"}{end}' 2>&1
REMOTE
