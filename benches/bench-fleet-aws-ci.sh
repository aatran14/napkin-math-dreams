#!/usr/bin/env bash
# Launch one AWS bench-worker per instance type in parallel; fail if any fails.
# Each worker appends to $REPO_DIR/data/results.csv (header seeded here).
set -euo pipefail

ALL_MACHINES=(c7i.4xlarge c7a.4xlarge c7g.4xlarge)
MACHINES=("${@:-${ALL_MACHINES[@]}}")
REPO_DIR="${REPO_DIR:-$(cd "$(dirname "$0")/.." && pwd)}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WORKER="$SCRIPT_DIR/bench-worker-aws.sh"
CSV="$REPO_DIR/data/results.csv"

mkdir -p "$REPO_DIR/data"
COMMIT="$(git -C "$REPO_DIR" rev-parse --short HEAD 2>/dev/null || echo unknown)"
TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "date,machine,cpu,config,operation,latency_ns,throughput_bytes_s,commit" > "$CSV"

PIDS=(); LOGS=()
for machine in "${MACHINES[@]}"; do
  LOG="/tmp/bench-aws-${machine}.log"; LOGS+=("$LOG")
  AWS_REGION="${AWS_REGION:-}" REPO_DIR="$REPO_DIR" TIMESTAMP="$TIMESTAMP" COMMIT="$COMMIT" \
    "$WORKER" "$machine" > "$LOG" 2>&1 &
  PIDS+=($!); echo "Started $machine (pid $!)"
done

FAILED=0
for i in "${!PIDS[@]}"; do
  if wait "${PIDS[$i]}"; then
    echo "OK: ${MACHINES[$i]}"
  else
    echo "FAIL: ${MACHINES[$i]} (see ${LOGS[$i]})"; cat "${LOGS[$i]}" >&2; FAILED=1
  fi
done

[[ "$FAILED" -eq 0 ]] || { echo "Some AWS benchmarks failed"; exit 1; }
echo "All AWS benchmarks complete"
