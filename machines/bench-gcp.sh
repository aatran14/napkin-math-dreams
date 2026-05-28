#!/usr/bin/env bash
# Provision a GCP instance, run benchmarks, pull results, and tear down.
#
# Usage:
#   ./machines/bench-gcp.sh c4d-standard-8-lssd        # AMD
#   ./machines/bench-gcp.sh c4a-standard-8-lssd        # ARM
#   ./machines/bench-gcp.sh c4-standard-8-lssd         # Intel
#
# Requires: gcloud CLI authenticated, gh CLI authenticated on the VM
# (gh auth login runs interactively — you'll need to complete the device flow)
set -euo pipefail

MACHINE_TYPE="${1:?Usage: bench-gcp.sh <machine-type>}"
ZONE="${NAPKIN_GCP_ZONE:-us-east1-b}"
NAME="bench-${MACHINE_TYPE%%-standard*}"
REPO="aatran14/napkin-math-dreams"

# ARM instances need arm64 image
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
  cargo build --release --bin daily
"

echo "=== mounting local SSD ==="
gcloud compute ssh "$NAME" --zone="$ZONE" --command="
  DEVICE=\$(lsblk -dno NAME,TYPE | grep disk | grep -v \$(findmnt -n -o SOURCE / | sed 's|/dev/||;s|p[0-9]*||') | head -1 | awk '{print \$1}')
  if [ -n \"\$DEVICE\" ]; then
    sudo mkfs.ext4 -F /dev/\$DEVICE
    sudo mkdir -p /mnt/localssd
    sudo mount /dev/\$DEVICE /mnt/localssd
    sudo chmod 777 /mnt/localssd
    echo \"mounted /dev/\$DEVICE at /mnt/localssd\"
  else
    echo \"WARNING: no local SSD found, using /tmp\"
  fi
"

echo "=== running benchmarks ==="
gcloud compute ssh "$NAME" --zone="$ZONE" --command="
  source ~/.cargo/env
  cd napkin-math-dreams
  if mountpoint -q /mnt/localssd; then
    NAPKIN_BENCH_FILE=/mnt/localssd/napkin_daily.bin
  else
    NAPKIN_BENCH_FILE=/tmp/napkin_daily.bin
  fi
  NAPKIN_MACHINE=gcp-$MACHINE_TYPE NAPKIN_CONFIG=baseline NAPKIN_BENCH_FILE=\$NAPKIN_BENCH_FILE cargo run --release --bin daily
"

echo "=== pulling results ==="
gcloud compute scp "$NAME":~/napkin-math-dreams/data/dead.csv "./data/${NAME}.csv" --zone="$ZONE"

echo ""
echo "=== done! results in data/${NAME}.csv ==="
# cleanup runs automatically via trap
