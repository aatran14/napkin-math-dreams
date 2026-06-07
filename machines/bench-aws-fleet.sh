#!/usr/bin/env bash
# AWS: one instance at a time (Intel → AMD → ARM). Runs attached so you see it live.
# Usage: ./machines/bench-aws-fleet.sh [instance-type ...]
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
BENCH="$REPO_DIR/machines/bench-aws.sh"

MACHINES=("$@")
(( ${#MACHINES[@]} )) || MACHINES=(c7i.12xlarge c7a.12xlarge c7g.12xlarge)

for machine in "${MACHINES[@]}"; do
  "$BENCH" "$machine"
done

echo ""
echo "=== AWS fleet done ==="
