#!/usr/bin/env bash
# Single-machine benchmark worker. Called by bench-fleet.sh, one per tmux pane.
# Usage: _bench-worker.sh <machine-type>
set -euo pipefail

MACHINE_TYPE="$1"
NAME="bench-${MACHINE_TYPE%%-standard*}"
ZONE="${ZONE:-us-east1-b}"
REPO_DIR="${REPO_DIR:-.}"

if [[ "$MACHINE_TYPE" == *c4a* ]]; then
  IMAGE_FAMILY="ubuntu-2404-lts-arm64"
else
  IMAGE_FAMILY="ubuntu-2404-lts-amd64"
fi

cleanup() {
  echo ""
  echo "=== deleting $NAME ==="
  gcloud compute instances delete "$NAME" --zone="$ZONE" --quiet 2>/dev/null || true
}
trap cleanup EXIT

echo "=== $NAME ($MACHINE_TYPE) ==="

echo "creating VM..."
gcloud compute instances create "$NAME" \
  --zone="$ZONE" \
  --machine-type="$MACHINE_TYPE" \
  --image-family="$IMAGE_FAMILY" \
  --image-project=ubuntu-os-cloud

echo "waiting for SSH..."
for i in $(seq 1 30); do
  if gcloud compute ssh "$NAME" --zone="$ZONE" --command="echo ready" 2>/dev/null; then
    break
  fi
  sleep 5
done

echo "installing deps..."
gcloud compute ssh "$NAME" --zone="$ZONE" --command="
  sudo apt-get update -qq
  sudo apt-get install -y -qq build-essential pkg-config libssl-dev
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
"

echo "uploading repo..."
gcloud compute scp --recurse --zone="$ZONE" --compress \
  "$REPO_DIR" "$NAME":~/napkin-math-dreams

echo "building..."
gcloud compute ssh "$NAME" --zone="$ZONE" --command="
  source ~/.cargo/env
  cd napkin-math-dreams
  cargo build --release --bin daily
"

echo "mounting local SSD..."
gcloud compute ssh "$NAME" --zone="$ZONE" --command="
  DEVICE=\$(lsblk -dno NAME,TYPE | grep disk | grep -v \$(findmnt -n -o SOURCE / | sed 's|/dev/||;s|p[0-9]*||') | head -1 | awk '{print \$1}')
  if [ -n \"\$DEVICE\" ]; then
    sudo mkfs.ext4 -F /dev/\$DEVICE
    sudo mkdir -p /mnt/localssd
    sudo mount /dev/\$DEVICE /mnt/localssd
    sudo chmod 777 /mnt/localssd
  fi
"

echo "running benchmarks..."
gcloud compute ssh "$NAME" --zone="$ZONE" --command="
  source ~/.cargo/env
  cd napkin-math-dreams
  if mountpoint -q /mnt/localssd; then
    BENCH_FILE=/mnt/localssd/napkin_daily.bin
  else
    BENCH_FILE=/tmp/napkin_daily.bin
  fi
  NAPKIN_MACHINE=gcp-$MACHINE_TYPE NAPKIN_CONFIG=baseline NAPKIN_BENCH_FILE=\$BENCH_FILE cargo run --release --bin daily
"

echo "pulling results..."
gcloud compute scp "$NAME":~/napkin-math-dreams/data/dead.csv "$REPO_DIR/data/${NAME}.csv" --zone="$ZONE"

echo ""
echo "=== $NAME done! results in data/${NAME}.csv ==="
