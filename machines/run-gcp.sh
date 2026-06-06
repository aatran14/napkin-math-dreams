#!/usr/bin/env bash
# Spin up a GCP VM, build, run a binary, and tear down.
#
# Usage:
#   ./machines/run-gcp.sh verify_memory
#   ./machines/run-gcp.sh verify_memory c4-standard-4
#   ./machines/run-gcp.sh verify_memory c4a-standard-4    # ARM
#
set -euo pipefail

BIN="${1:?Usage: run-gcp.sh <binary-name> [machine-type]}"
MACHINE_TYPE="${2:-n1-standard-1}"
ZONE="${NAPKIN_GCP_ZONE:-us-central1-a}"
NAME="run-${BIN//_/-}-$(date +%s | tail -c 5)"
REPO="aatran14/napkin-math-dreams"

if [[ "$MACHINE_TYPE" == *c4a* ]]; then
  IMAGE_FAMILY="ubuntu-2404-lts-arm64"
else
  IMAGE_FAMILY="ubuntu-2404-lts-amd64"
fi

cleanup() {
  echo ""
  echo "=== cleaning up ==="
  gcloud compute instances delete "$NAME" --zone="$ZONE" --quiet || true
}
trap cleanup EXIT

echo "=== creating $NAME ($MACHINE_TYPE) in $ZONE ==="
gcloud compute instances create "$NAME" \
  --zone="$ZONE" \
  --machine-type="$MACHINE_TYPE" \
  --image-family="$IMAGE_FAMILY" \
  --image-project=ubuntu-os-cloud

echo "=== waiting for SSH ==="
for i in $(seq 1 30); do
  if gcloud compute ssh "$NAME" --zone="$ZONE" --command="echo ready" 2>/dev/null; then
    break
  fi
  sleep 5
done

echo "=== installing deps ==="
gcloud compute ssh "$NAME" --zone="$ZONE" --command="
  sudo apt-get update -qq
  sudo apt-get install -y -qq build-essential pkg-config libssl-dev gh
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
"

echo ""
echo "=== gh auth needed ==="
echo "Run this in another terminal:"
echo "  gcloud compute ssh $NAME --zone=$ZONE"
echo "  gh auth login"
echo ""
read -p "Press Enter once gh auth is done on the VM..."

echo "=== cloning and building ==="
gcloud compute ssh "$NAME" --zone="$ZONE" --command="
  source ~/.cargo/env
  gh repo clone $REPO
  cd napkin-math-dreams
  sudo sysctl -w kernel.perf_event_paranoid=-1
  cargo build --release --bin $BIN
"

echo "=== running $BIN ==="
gcloud compute ssh "$NAME" --zone="$ZONE" --command="
  source ~/.cargo/env
  cd napkin-math-dreams
  ./target/release/$BIN
"

echo ""
echo "=== done ==="
