#!/usr/bin/env bash
# Copy this repo to your GCP VM, install Rust if needed, run one binary.
#
#   ./run-on-vm.sh memory
#
# Env: NAPKIN_VM=my-vm  NAPKIN_GCP_ZONE=us-central1-a

set -euo pipefail

VM="${NAPKIN_VM:-my-vm}"
ZONE="${NAPKIN_GCP_ZONE:-us-central1-a}"
BIN="${1:?usage: ./run-on-vm.sh <binary-name>  e.g. verify_memory}"
ROOT="$(cd "$(dirname "$0")" && pwd)"
DIR="napkin-math-dreams"

echo "syncing to ${VM}..."
COPYFILE_DISABLE=1 tar czf - -C "$ROOT" --exclude target --exclude .git . | \
  gcloud compute ssh "$VM" --zone="$ZONE" -- \
    "mkdir -p ~/${DIR} && tar xzf - -C ~/${DIR} 2>/dev/null"

echo "running ${BIN} (first run installs Rust and compiles — can take ~10 min)..."
gcloud compute ssh "$VM" --zone="$ZONE" --command="
set -euo pipefail
if ! command -v cargo >/dev/null 2>&1; then
  sudo apt-get update -qq
  sudo apt-get install -y -qq build-essential pkg-config libssl-dev
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
fi
source ~/.cargo/env
cd ~/${DIR}
sudo sysctl -w kernel.perf_event_paranoid=-1 2>/dev/null || true
cargo run --release --bin ${BIN}
"
