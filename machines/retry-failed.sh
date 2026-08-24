#!/usr/bin/env bash
# Retry, once, the machines an earlier fleet pass logged as failed.
#
# The fleet scripts append "<cloud>\t<machine>\t<exit>" to $NAPKIN_FAILED_LOG
# and keep going instead of aborting. This runs those machines one more time.
# Anything that fails again lands back in the log and gets reported at the end
# of the run.
#
# This makes exactly one pass, so the night is bounded at two provisions per
# machine no matter how badly things go. If nothing at all succeeded, the fault
# is systemic rather than transient, so we skip the retry instead of paying
# twice to watch it fail again.
#
# Usage: ./machines/retry-failed.sh
set -uo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
FAILED_LOG="${NAPKIN_FAILED_LOG:-$REPO_DIR/data/failed-machines.txt}"
RESULTS="${NAPKIN_RESULTS_CSV:-$REPO_DIR/data/dead.csv}"

if [ ! -s "$FAILED_LOG" ]; then
  echo "=== every machine reported, nothing to retry ==="
  exit 0
fi

if [ ! -s "$RESULTS" ]; then
  echo "=== fleet-wide failure, not retrying ==="
  echo "no machine produced results, so this is not a transient fault"
  exit 0
fi

# Take this pass's failures; the fleet scripts refill the log with whatever
# fails a second time.
WORKLIST="$(mktemp)"
mv "$FAILED_LOG" "$WORKLIST"

gcp=()
aws=()
while IFS=$'\t' read -r cloud machine status || [ -n "${cloud:-}" ]; do
  [ -n "${machine:-}" ] || continue
  case "$cloud" in
    gcp) gcp+=("$machine") ;;
    aws) aws+=("$machine") ;;
  esac
done < "$WORKLIST"
rm -f "$WORKLIST"

echo ""
echo "=== retrying ${#gcp[@]} gcp, ${#aws[@]} aws ==="

if (( ${#gcp[@]} )); then
  bash "$REPO_DIR/machines/bench-gcp-fleet.sh" "${gcp[@]}"
fi
if (( ${#aws[@]} )); then
  bash "$REPO_DIR/machines/bench-aws-fleet.sh" "${aws[@]}"
fi

echo ""
if [ -s "$FAILED_LOG" ]; then
  echo "=== retry done, still missing ==="
  cat "$FAILED_LOG"
else
  echo "=== retry done, recovered every machine ==="
fi
exit 0
