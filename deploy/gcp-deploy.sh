#!/usr/bin/env bash
# Zentra VM Deploy Script
# Called by Cloud Build via SSH. Pulls new images, restarts services,
# verifies health, and cleans up old images.
#
# Usage: deploy.sh <IMAGE_TAG> <REGION>
# Example: deploy.sh abc1234 us-central1

set -euo pipefail

IMAGE_TAG="${1:?Usage: deploy.sh <IMAGE_TAG> <REGION>}"
REGION="${2:-us-central1}"
REGISTRY="${REGION}-docker.pkg.dev"
PROJECT_ID=$(curl -sf -H "Metadata-Flavor: Google" \
  "http://metadata.google.internal/computeMetadata/v1/project/project-id" 2>/dev/null || echo "zentra-prod")

BACKEND_IMAGE="${REGISTRY}/${PROJECT_ID}/zentra/zentra-backend:${IMAGE_TAG}"
FRONTEND_IMAGE="${REGISTRY}/${PROJECT_ID}/zentra/zentra-frontend:${IMAGE_TAG}"

COMPOSE_DIR="/opt/zentra"
cd "${COMPOSE_DIR}"

echo "==> Pulling secrets from Secret Manager"
export DATABASE_URL=$(gcloud secrets versions access latest --secret=zentra-database-url 2>/dev/null || echo "${DATABASE_URL}")
export JWT_SECRET=$(gcloud secrets versions access latest --secret=zentra-jwt-secret 2>/dev/null || echo "${JWT_SECRET}")
export STRIPE_SECRET_KEY=$(gcloud secrets versions access latest --secret=zentra-stripe-secret-key 2>/dev/null || echo "${STRIPE_SECRET_KEY:-}")
export STRIPE_WEBHOOK_SECRET=$(gcloud secrets versions access latest --secret=zentra-stripe-webhook-secret 2>/dev/null || echo "${STRIPE_WEBHOOK_SECRET:-}")
export STRIPE_PRICE_STARTER=$(gcloud secrets versions access latest --secret=zentra-stripe-price-starter 2>/dev/null || echo "${STRIPE_PRICE_STARTER:-}")
export STRIPE_PRICE_PROFESSIONAL=$(gcloud secrets versions access latest --secret=zentra-stripe-price-professional 2>/dev/null || echo "${STRIPE_PRICE_PROFESSIONAL:-}")
export STRIPE_PRICE_ENTERPRISE=$(gcloud secrets versions access latest --secret=zentra-stripe-price-enterprise 2>/dev/null || echo "${STRIPE_PRICE_ENTERPRISE:-}")

echo "==> Pulling new images"
docker pull "${BACKEND_IMAGE}"
docker pull "${FRONTEND_IMAGE}"

echo "==> Tagging as :current"
docker tag "${BACKEND_IMAGE}" zentra-backend:current
docker tag "${FRONTEND_IMAGE}" zentra-frontend:current

echo "==> Restarting backend"
docker compose stop zentra-backend || true
docker compose up -d zentra-backend

echo "==> Waiting for backend health check"
for i in $(seq 1 30); do
  if curl -sf http://localhost:5006/api/health > /dev/null 2>&1; then
    echo "   Backend healthy after attempt ${i}"
    break
  fi
  if [ "${i}" -eq 30 ]; then
    echo "ERROR: Backend health check failed"
    exit 1
  fi
  sleep 3
done

echo "==> Restarting frontend"
docker compose stop zentra-frontend || true
docker compose up -d zentra-frontend

echo "==> Cleaning up old images (keeping last 3)"
docker images --format '{{.Repository}}:{{.Tag}} {{.CreatedAt}}' \
  | grep 'zentra-backend\|zentra-frontend' \
  | grep -v ':current\|:latest' \
  | sort -k2 -r \
  | tail -n +4 \
  | awk '{print $1}' \
  | xargs -r docker rmi --force 2>/dev/null || true

echo "==> Deploy complete: backend=${BACKEND_IMAGE} frontend=${FRONTEND_IMAGE}"
echo "==> Image tag: ${IMAGE_TAG}"
date -u +"%Y-%m-%dT%H:%M:%SZ"
