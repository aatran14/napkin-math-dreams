#!/usr/bin/env bash
# AWS: one instance at a time (Intel → AMD → ARM). Runs attached so you see it live.
# Usage: ./machines/bench-aws-fleet.sh [instance-type ...]
#
# An instance that fails is appended to $NAPKIN_FAILED_LOG and the fleet moves on,
# so one bad instance doesn't discard the results from the others.
# Exits non-zero only when every instance failed.
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BENCH="$REPO_DIR/machines/bench-aws.sh"
FAILED_LOG="${NAPKIN_FAILED_LOG:-$REPO_DIR/data/failed-machines.txt}"

MACHINES=("$@")
(( ${#MACHINES[@]} )) || MACHINES=(c7i.4xlarge c7a.4xlarge c7g.4xlarge)

mkdir -p "$(dirname "$FAILED_LOG")"

ok=0
failed=0
for machine in "${MACHINES[@]}"; do
  if "$BENCH" "$machine"; then
    ok=$((ok + 1))
  else
    status=$?
    failed=$((failed + 1))
    printf 'aws\t%s\t%s\n' "$machine" "$status" >> "$FAILED_LOG"
    echo "(exit $status) aws $machine failed"
    echo "machine failure logged . continuing with the rest of the fleet"
  fi
done

echo ""
echo "=== AWS fleet done: $ok ok, $failed failed ==="
(( ok > 0 || failed == 0 ))
