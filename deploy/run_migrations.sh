#!/usr/bin/env bash
set -euo pipefail
MIGDIR="/store/Zentra/migrations"
LAST_APPLIED="20250101000011"

cd "$MIGDIR"
for f in $(ls *.sql | sort); do
  ver=$(echo "$f" | cut -d_ -f1)
  if [ "$ver" -gt "$LAST_APPLIED" ]; then
    echo "--- Applying: $f"
    psql -h localhost -p 5434 -U postgres -d zentra -f "$f" 2>&1 | grep -v '^$' | tail -5
    echo "    done."
  fi
done
echo "All pending migrations applied."
