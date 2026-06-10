#!/usr/bin/env bash
# One GCP VM: create -> run `daily` binary -> append CSV -> delete.
# Called per-machine by bench-fleet-ci.sh. Env in: ZONE, REPO_DIR, COMMIT, TIMESTAMP.
set -euo pipefail

MACHINE_TYPE="$1"
NAME="bench-${MACHINE_TYPE%%-standard*}"
ZONE="${ZONE:-us-east1-b}"
REPO_DIR="${REPO_DIR:-.}"
CSV="$REPO_DIR/data/results.csv"

if [[ "$MACHINE_TYPE" == *c4a* ]]; then
  IMAGE_FAMILY="ubuntu-2404-lts-arm64"
else
  IMAGE_FAMILY="ubuntu-2404-lts-amd64"
fi

cleanup() { gcloud compute instances delete "$NAME" --zone="$ZONE" --quiet 2>/dev/null || true; }
trap cleanup EXIT

echo "=== $NAME ($MACHINE_TYPE) ==="
gcloud compute instances create "$NAME" \
  --zone="$ZONE" \
  --machine-type="$MACHINE_TYPE" \
  --image-family="$IMAGE_FAMILY" \
  --image-project=ubuntu-os-cloud

for _ in $(seq 1 30); do
  gcloud compute ssh "$NAME" --zone="$ZONE" --command="echo ready" 2>/dev/null && break
  sleep 5
done

# .git and data excluded; the SHA is passed in via COMMIT instead.
tar czf "/tmp/${NAME}.tar.gz" -C "$REPO_DIR" --exclude=target --exclude=.git --exclude=data .
gcloud compute scp --zone="$ZONE" "/tmp/${NAME}.tar.gz" "$NAME":~/repo.tar.gz
rm -f "/tmp/${NAME}.tar.gz"
gcloud compute ssh "$NAME" --zone="$ZONE" --command="mkdir -p ~/napkin-math-dreams/data && tar xzf ~/repo.tar.gz -C ~/napkin-math-dreams"

gcloud compute ssh "$NAME" --zone="$ZONE" --command="
  sudo apt-get update -qq
  sudo apt-get install -y -qq build-essential pkg-config libssl-dev
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source ~/.cargo/env
  cd napkin-math-dreams && cargo build --release --bin daily
"

# Mount a local SSD if the machine has one (lssd types do).
gcloud compute ssh "$NAME" --zone="$ZONE" --command="
  DEVICE=\$(lsblk -dno NAME,TYPE | grep disk | grep -v \$(findmnt -n -o SOURCE / | sed 's|/dev/||;s|p[0-9]*||') | head -1 | awk '{print \$1}')
  if [ -n \"\$DEVICE\" ]; then
    sudo mkfs.ext4 -F /dev/\$DEVICE && sudo mkdir -p /mnt/localssd && sudo mount /dev/\$DEVICE /mnt/localssd && sudo chmod 777 /mnt/localssd
  fi
"

gcloud compute ssh "$NAME" --zone="$ZONE" --command="cd napkin-math-dreams && sudo bash tuning/bench_stable.sh" || true

gcloud compute ssh "$NAME" --zone="$ZONE" --command="
  source ~/.cargo/env
  cd napkin-math-dreams
  if mountpoint -q /mnt/localssd; then BENCH_FILE=/mnt/localssd/napkin_daily.bin; else BENCH_FILE=/tmp/napkin_daily.bin; fi
  NAPKIN_MACHINE=gcp-$MACHINE_TYPE NAPKIN_CONFIG=bench_stable NAPKIN_BENCH_FILE=\$BENCH_FILE \
    NAPKIN_COMMIT='${COMMIT:-}' NAPKIN_TIMESTAMP='${TIMESTAMP:-}' NAPKIN_CSV=data/results.csv \
    cargo run --release --bin daily
"

gcloud compute scp "$NAME":~/napkin-math-dreams/data/results.csv "/tmp/${NAME}.csv" --zone="$ZONE"

# Append into the shared results.csv (skip header); lockfile so parallel workers don't interleave.
LOCK="$REPO_DIR/data/.results.lock"
while ! mkdir "$LOCK" 2>/dev/null; do sleep 0.5; done
tail -n +2 "/tmp/${NAME}.csv" >> "$CSV"
rmdir "$LOCK"
rm -f "/tmp/${NAME}.csv"
echo "=== $NAME done ==="
