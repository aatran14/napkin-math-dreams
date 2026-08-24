#!/usr/bin/env bash
# Single-machine benchmark worker. Called by bench-gcp-fleet.sh.
# Usage: _bench-worker.sh <machine-type>
#
# The benchmark VM gets a prebuilt `daily` binary and the tuning scripts, and
# nothing else. No apt, no rustup, no compiler. Building here used to mean every
# run depended on the Ubuntu mirror being healthy, which is what cost us whole
# nights when the arm64 ports mirror started 503ing.
#
# Requires $NAPKIN_BIN_DIR/daily-<arch>/daily, built by the workflow's build job.
set -euo pipefail

MACHINE_TYPE="$1"
NAME="bench-${MACHINE_TYPE%%-standard*}"
ZONE="${ZONE:-us-east1-b}"
REPO_DIR="${REPO_DIR:-.}"
BIN_DIR="${NAPKIN_BIN_DIR:-$REPO_DIR/bin}"

if [[ "$MACHINE_TYPE" == *c4a* ]]; then
  IMAGE_FAMILY="ubuntu-2404-lts-arm64"
  ARCH="arm64"
else
  IMAGE_FAMILY="ubuntu-2404-lts-amd64"
  ARCH="amd64"
fi

BIN="$BIN_DIR/daily-$ARCH/daily"
if [ ! -f "$BIN" ]; then
  echo "missing prebuilt binary: $BIN" >&2
  echo "build it with: cargo build --release --bin daily   (on a linux/$ARCH host)" >&2
  exit 1
fi

cleanup() {
  echo ""
  echo "=== tearing down $NAME ==="
  gcloud compute instances delete "$NAME" --zone="$ZONE" --quiet 2>/dev/null || true
}
trap cleanup EXIT

echo "=== $NAME ($MACHINE_TYPE) ==="

create_vm() {
  gcloud compute instances create "$NAME" \
    --zone="$ZONE" \
    --machine-type="$MACHINE_TYPE" \
    --image-family="$IMAGE_FAMILY" \
    --image-project=ubuntu-os-cloud \
    "$@"
}

# The 3h auto-shutoff is a safety net against leaked VMs, not a requirement, so
# it must never be the reason a run doesn't start. If this zone or machine type
# won't take the flags, provision without them.
echo "provisioning VM..."
if ! create_vm --max-run-duration=10800s --instance-termination-action=DELETE; then
  echo "auto-shutoff not accepted here, provisioning without it"
  create_vm
fi

echo "waiting for SSH..."
for i in $(seq 1 30); do
  if gcloud compute ssh "$NAME" --zone="$ZONE" --command="echo ready" 2>/dev/null; then
    break
  fi
  sleep 5
done

# daily reads the benchmarks/ TOML tree at runtime to decide what to run and
# panics if it isn't there, so it ships with the binary. tuning/ is the
# bench_stable.sh the VM runs before measuring. That's the whole payload.
echo "uploading binary and benchmark definitions..."
tar czf "/tmp/${NAME}-payload.tar.gz" -C "$REPO_DIR" benchmarks tuning
gcloud compute scp "/tmp/${NAME}-payload.tar.gz" "$NAME":~/payload.tar.gz --zone="$ZONE"
gcloud compute scp "$BIN" "$NAME":~/daily --zone="$ZONE"
rm -f "/tmp/${NAME}-payload.tar.gz"
gcloud compute ssh "$NAME" --zone="$ZONE" --command="
  set -e
  mkdir -p ~/napkin/data
  tar xzf ~/payload.tar.gz -C ~/napkin
  mv ~/daily ~/napkin/daily
  chmod +x ~/napkin/daily
  test -d ~/napkin/benchmarks
  test -f ~/napkin/tuning/bench_stable.sh
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

echo "tuning VM for stable benchmarks..."
gcloud compute ssh "$NAME" --zone="$ZONE" --command="
  cd napkin
  sudo bash tuning/bench_stable.sh
"

echo "running benchmarks..."
gcloud compute ssh "$NAME" --zone="$ZONE" --command="
  cd napkin
  if mountpoint -q /mnt/localssd; then
    BENCH_FILE=/mnt/localssd/napkin_daily.bin
  else
    BENCH_FILE=/tmp/napkin_daily.bin
  fi
  NAPKIN_MACHINE=gcp-$MACHINE_TYPE NAPKIN_CONFIG=bench_stable NAPKIN_BENCH_FILE=\$BENCH_FILE ./daily
"

echo "pulling results..."
gcloud compute scp "$NAME":~/napkin/data/dead.csv "/tmp/${NAME}.csv" --zone="$ZONE"

# merge into dead.csv (skip header, append data rows)
# use lockfile to prevent concurrent panes from interleaving
LOCKFILE="$REPO_DIR/data/.dead.csv.lock"
while ! mkdir "$LOCKFILE" 2>/dev/null; do sleep 0.5; done
tail -n +2 "/tmp/${NAME}.csv" >> "$REPO_DIR/data/dead.csv"
rmdir "$LOCKFILE"
rm -f "/tmp/${NAME}.csv"

echo ""
echo "=== $NAME done! results merged into data/dead.csv ==="
