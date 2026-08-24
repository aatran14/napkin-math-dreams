#!/usr/bin/env bash
# GCP: one VM at a time (Intel → AMD → ARM). Runs attached so you see it live.
# Usage: ./machines/bench-gcp-fleet.sh [machine-type ...]
#
# A machine that fails is appended to $NAPKIN_FAILED_LOG and the fleet moves on,
# so a zone stockout on one VM doesn't discard the results from the others.
# Exits non-zero only when every machine failed.
set -euo pipefail

ZONE="${NAPKIN_GCP_ZONE:-us-east1-b}"
REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
WORKER="$REPO_DIR/machines/_bench-worker.sh"
FAILED_LOG="${NAPKIN_FAILED_LOG:-$REPO_DIR/data/failed-machines.txt}"

MACHINES=("$@")
(( ${#MACHINES[@]} )) || MACHINES=(c4-standard-16-lssd c4d-standard-16-lssd c4a-standard-16-lssd)

mkdir -p "$(dirname "$FAILED_LOG")"

ok=0
failed=0
for machine in "${MACHINES[@]}"; do
  if ZONE="$ZONE" REPO_DIR="$REPO_DIR" "$WORKER" "$machine"; then
    ok=$((ok + 1))
  else
    status=$?
    failed=$((failed + 1))
    printf 'gcp\t%s\t%s\n' "$machine" "$status" >> "$FAILED_LOG"
    echo "(exit $status) gcp $machine failed"
    echo "machine failure logged . continuing with the rest of the fleet"
  fi
done

echo ""
echo "=== GCP fleet done: $ok ok, $failed failed ==="
(( ok > 0 || failed == 0 ))
