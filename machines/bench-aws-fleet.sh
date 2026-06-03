#!/usr/bin/env bash
# Run benchmarks across the entire AWS fleet in parallel, one tmux pane per machine.
#
# Usage:
#   ./machines/bench-aws-fleet.sh                              # all AWS machines
#   ./machines/bench-aws-fleet.sh c7i.2xlarge c7g.2xlarge      # specific ones
set -euo pipefail

ALL_MACHINES=(
  c7i.2xlarge
  c7a.2xlarge
  c7g.2xlarge
)

MACHINES=("${@:-${ALL_MACHINES[@]}}")
REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
SESSION="napkin-aws-fleet"

# Kill old session if it exists
tmux kill-session -t "$SESSION" 2>/dev/null || true

# First machine gets the initial window
FIRST="${MACHINES[0]}"
FIRST_NAME="bench-${FIRST//\./-}"
tmux new-session -d -s "$SESSION" -n "$FIRST_NAME" \
  "$REPO_DIR/machines/bench-aws.sh $FIRST; echo ''; echo 'Press Enter to close...'; read"

# Remaining machines get split panes
for machine in "${MACHINES[@]:1}"; do
  tmux split-window -t "$SESSION" \
    "$REPO_DIR/machines/bench-aws.sh $machine; echo ''; echo 'Press Enter to close...'; read"
  tmux select-layout -t "$SESSION" tiled
done

echo "Attaching to tmux session '$SESSION' with ${#MACHINES[@]} panes."
echo "Ctrl-B D to detach, Ctrl-B [ to scroll."
tmux attach -t "$SESSION"
