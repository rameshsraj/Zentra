#!/usr/bin/env bash
# GCP Budget Alert Setup — $150 threshold (half of $300 free credit)
# Run once after project setup.
#
# Prerequisites:
#   gcloud auth login
#   gcloud config set project YOUR_PROJECT_ID
#   gcloud services enable billingbudgets.googleapis.com

set -euo pipefail

PROJECT_ID=$(gcloud config get-value project)
BILLING_ACCOUNT=$(gcloud billing projects describe "$PROJECT_ID" --format='value(billingAccountName)' | sed 's|billingAccounts/||')
ALERT_EMAIL="${1:-$(gcloud config get-value account)}"

echo "==> Creating budget alert for project: ${PROJECT_ID}"
echo "    Billing account: ${BILLING_ACCOUNT}"
echo "    Alert email: ${ALERT_EMAIL}"

gcloud billing budgets create \
  --billing-account="${BILLING_ACCOUNT}" \
  --display-name="Zentra - $150 Alert" \
  --budget-amount=300USD \
  --threshold-rule=percent=0.50,basis=current-spend \
  --threshold-rule=percent=0.80,basis=current-spend \
  --threshold-rule=percent=1.00,basis=current-spend \
  --all-updates-rule-monitoring-notification-channels="" \
  --filter-projects="projects/${PROJECT_ID}"

echo "==> Budget alert created. You will be notified at 50%, 80%, and 100% of \$300."
echo "    (50% = \$150, 80% = \$240, 100% = \$300)"
