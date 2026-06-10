#!/usr/bin/env bash
# Launch one bench-worker per machine type in parallel; fail if any worker fails.
# Each worker appends to $REPO_DIR/data/results.csv (header seeded here).
set -euo pipefail

ALL_MACHINES=(
  c4-standard-16-lssd
  c4d-standard-16-lssd
  c4a-standard-16-lssd
)
MACHINES=("${@:-${ALL_MACHINES[@]}}")
ZONE="${NAPKIN_GCP_ZONE:-us-east1-b}"
REPO_DIR="${REPO_DIR:-$(cd "$(dirname "$0")/.." && pwd)}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WORKER="$SCRIPT_DIR/bench-worker.sh"
CSV="$REPO_DIR/data/results.csv"

mkdir -p "$REPO_DIR/data"
COMMIT="$(git -C "$REPO_DIR" rev-parse --short HEAD 2>/dev/null || echo unknown)"
TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "date,machine,cpu,config,operation,latency_ns,throughput_bytes_s,commit" > "$CSV"

PIDS=(); LOGS=()
for machine in "${MACHINES[@]}"; do
  LOG="/tmp/bench-${machine}.log"; LOGS+=("$LOG")
  ZONE="$ZONE" REPO_DIR="$REPO_DIR" TIMESTAMP="$TIMESTAMP" COMMIT="$COMMIT" \
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

[[ "$FAILED" -eq 0 ]] || { echo "Some benchmarks failed"; exit 1; }
echo "All benchmarks complete"
