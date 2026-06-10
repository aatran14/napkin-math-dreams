#!/usr/bin/env bash
# Same-zone network: two matched GCP VMs, iperf3 throughput + TCP RTT.
# Usage: ./machines/probe-net-pair-gcp.sh [machine-type]
# Fleet: NAPKIN_CSV set → append network_same_zone rows to dead.csv
set -euo pipefail

MACHINE_TYPE="${1:-e2-standard-2}"
ZONE="${NAPKIN_GCP_ZONE:-us-east1-b}"
SUFFIX="${NAPKIN_RUN_SUFFIX:-$$}"
SRV="net-probe-srv-${SUFFIX}"
CLI="net-probe-cli-${SUFFIX}"
REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
RESULTS="/tmp/net-same-zone-${SUFFIX}.out"

if [[ "$MACHINE_TYPE" == *c4a* ]]; then
  IMAGE_FAMILY="ubuntu-2404-lts-arm64"
else
  IMAGE_FAMILY="ubuntu-2404-lts-amd64"
fi

cleanup() {
  echo "=== tearing down ==="
  gcloud compute instances delete "$SRV" "$CLI" --zone="$ZONE" --quiet 2>/dev/null || true
}
trap cleanup EXIT

echo "=== creating $SRV and $CLI ($MACHINE_TYPE) in $ZONE ==="
gcloud compute instances create "$SRV" "$CLI" \
  --zone="$ZONE" \
  --machine-type="$MACHINE_TYPE" \
  --image-family="$IMAGE_FAMILY" \
  --image-project=ubuntu-os-cloud

for vm in "$SRV" "$CLI"; do
  for i in $(seq 1 36); do
    if gcloud compute ssh "$vm" --zone="$ZONE" --command="echo ready" 2>/dev/null; then
      echo "$vm: ssh ok"
      break
    fi
    [ "$i" -eq 36 ] && { echo "$vm: ssh timeout"; exit 1; }
    sleep 10
  done
done

for vm in "$SRV" "$CLI"; do
  gcloud compute ssh "$vm" --zone="$ZONE" --command="
    sudo apt-get update -qq
    sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq iperf3 python3
  "
done

SRV_IP=$(gcloud compute instances describe "$SRV" --zone="$ZONE" --format='get(networkInterfaces[0].networkIP)')
echo "=== server internal IP: $SRV_IP ==="

if [ -z "${NAPKIN_CPU:-}" ]; then
  NAPKIN_CPU=$(gcloud compute ssh "$CLI" --zone="$ZONE" --command="lscpu 2>/dev/null | awk -F: '/Model name/{print \$2; exit}'" | sed 's/^[[:space:]]*//')
  export NAPKIN_CPU
fi

gcloud compute ssh "$SRV" --zone="$ZONE" --command="
  pkill iperf3 2>/dev/null || true
  nohup iperf3 -s > /tmp/iperf3-srv.log 2>&1 &
  sleep 2
  pgrep -a iperf3
"

gcloud compute scp --zone="$ZONE" \
  "$REPO_DIR/scripts/net-same-zone-measure.sh" \
  "$REPO_DIR/scripts/net-same-zone-rtt.py" \
  "$CLI":/tmp/

PARALLEL="${NET_PROBE_PARALLEL:-4}"
gcloud compute ssh "$CLI" --zone="$ZONE" --command="bash /tmp/net-same-zone-measure.sh $SRV_IP $PARALLEL" | tee "$RESULTS"

if [ -n "${NAPKIN_CSV:-}" ]; then
  # shellcheck source=scripts/net-same-zone-csv.sh
  source "$REPO_DIR/scripts/net-same-zone-csv.sh"
  net_same_zone_append_csv "$RESULTS"
fi

echo "=== GCP same-zone probe: OK ==="
