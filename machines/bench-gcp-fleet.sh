#!/usr/bin/env bash
# Run benchmarks across the entire GCP fleet in parallel, one tmux pane per machine.
#
# Usage:
#   ./machines/bench-fleet.sh                    # all GCP machines
#   ./machines/bench-fleet.sh c4d-standard-8-lssd c4a-standard-8-lssd  # specific ones
set -euo pipefail

ALL_MACHINES=(
  c4-standard-8-lssd
  c4d-standard-8-lssd
  c4a-standard-8-lssd
)

MACHINES=("${@:-${ALL_MACHINES[@]}}")
ZONE="${NAPKIN_GCP_ZONE:-us-east1-b}"
REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
SESSION="napkin-fleet"
WORKER="$REPO_DIR/machines/_bench-worker.sh"

# Kill old session if it exists
tmux kill-session -t "$SESSION" 2>/dev/null || true

# First machine gets the initial window
FIRST="${MACHINES[0]}"
FIRST_NAME="bench-${FIRST%%-standard*}"
tmux new-session -d -s "$SESSION" -n "$FIRST_NAME" \
  "ZONE=$ZONE REPO_DIR=$REPO_DIR $WORKER $FIRST; echo ''; echo 'Press Enter to close...'; read"

# Remaining machines get split panes
for machine in "${MACHINES[@]:1}"; do
  name="bench-${machine%%-standard*}"
  tmux split-window -t "$SESSION" \
    "ZONE=$ZONE REPO_DIR=$REPO_DIR $WORKER $machine; echo ''; echo 'Press Enter to close...'; read"
  tmux select-layout -t "$SESSION" tiled
done

echo "Attaching to tmux session '$SESSION' with ${#MACHINES[@]} panes."
echo "Ctrl-B D to detach, Ctrl-B [ to scroll."
tmux attach -t "$SESSION"
