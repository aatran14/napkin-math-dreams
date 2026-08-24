#!/usr/bin/env bash
# Same-zone network: two matched GCP VMs, iperf3 inside + outside VPC.
# Inside:  client → server networkIP (private path)
# Outside: client → server natIP (hairpin through public IP)
# Usage: ./machines/probe-net-pair-gcp.sh [machine-type]
# Fleet: NAPKIN_CSV set → append network_inside_vpc* / network_outside_vpc* rows
set -euo pipefail

MACHINE_TYPE="${1:-e2-standard-2}"
ZONE="${NAPKIN_GCP_ZONE:-us-east1-b}"
SUFFIX="${NAPKIN_RUN_SUFFIX:-$$}"
SRV="net-probe-srv-${SUFFIX}"
CLI="net-probe-cli-${SUFFIX}"
FW_RULE="napkin-iperf-${SUFFIX}"
PROBE_TAG="napkin-probe-${SUFFIX}"
REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
RESULTS_INSIDE="/tmp/net-inside-vpc-${SUFFIX}.out"
RESULTS_OUTSIDE="/tmp/net-outside-vpc-${SUFFIX}.out"

if [[ "$MACHINE_TYPE" == *c4a* ]]; then
  IMAGE_FAMILY="ubuntu-2404-lts-arm64"
else
  IMAGE_FAMILY="ubuntu-2404-lts-amd64"
fi

cleanup() {
  echo "=== tearing down ==="
  gcloud compute firewall-rules delete "$FW_RULE" --quiet 2>/dev/null || true
  gcloud compute instances delete "$SRV" "$CLI" --zone="$ZONE" --quiet 2>/dev/null || true
}
trap cleanup EXIT

echo "=== provisioning $SRV and $CLI ($MACHINE_TYPE) in $ZONE ==="
gcloud compute instances create "$SRV" "$CLI" \
  --zone="$ZONE" \
  --machine-type="$MACHINE_TYPE" \
  --image-family="$IMAGE_FAMILY" \
  --image-project=ubuntu-os-cloud \
  --tags="$PROBE_TAG"

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

# iperf3 is a system package, so unlike the compute benchmarks this pair can't
# be handed a prebuilt binary and must still talk to the Ubuntu mirror. Mirror
# 503s are what used to lose whole nights, and they pass in seconds, so retry
# instead of letting the first one end the probe.
echo "installing iperf3..."
for vm in "$SRV" "$CLI"; do
  gcloud compute ssh "$vm" --zone="$ZONE" --command="
    set -e
    for attempt in 1 2 3 4 5; do
      if sudo apt-get update -qq \
        && sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq iperf3 python3; then
        exit 0
      fi
      echo \"apt attempt \$attempt failed, retrying in \$((attempt * 10))s\"
      sleep \$((attempt * 10))
    done
    echo 'apt failed 5 times, giving up' >&2
    exit 1
  "
done

SRV_IP=$(gcloud compute instances describe "$SRV" --zone="$ZONE" --format='get(networkInterfaces[0].networkIP)')
SRV_NAT=$(gcloud compute instances describe "$SRV" --zone="$ZONE" --format='get(networkInterfaces[0].accessConfigs[0].natIP)')
CLI_NAT=$(gcloud compute instances describe "$CLI" --zone="$ZONE" --format='get(networkInterfaces[0].accessConfigs[0].natIP)')
NETWORK=$(gcloud compute instances describe "$SRV" --zone="$ZONE" --format='value(networkInterfaces[0].network)' | xargs basename)

echo "=== server internal IP: $SRV_IP ==="
echo "=== server nat IP:      $SRV_NAT ==="
echo "=== client nat IP:      $CLI_NAT ==="

if [ -z "$SRV_NAT" ] || [ -z "$CLI_NAT" ]; then
  echo "error: probe VMs need ephemeral external IPs (natIP)" >&2
  exit 1
fi

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
  "$CLI":/tmp/

PARALLEL="${NET_PROBE_PARALLEL:-4}"

echo "=== inside VPC: client → $SRV_IP ==="
gcloud compute ssh "$CLI" --zone="$ZONE" --command="bash /tmp/net-same-zone-measure.sh $SRV_IP $PARALLEL inside-vpc" | tee "$RESULTS_INSIDE"

echo "=== outside VPC: opening tcp/5201 from $CLI_NAT/32 to $PROBE_TAG ==="
gcloud compute firewall-rules create "$FW_RULE" \
  --network="$NETWORK" \
  --direction=INGRESS \
  --action=ALLOW \
  --rules=tcp:5201 \
  --source-ranges="${CLI_NAT}/32" \
  --target-tags="$PROBE_TAG" \
  --description="napkin net probe: iperf3 from client external IP"

echo "=== outside VPC: client → $SRV_NAT (hairpin) ==="
gcloud compute ssh "$CLI" --zone="$ZONE" --command="bash /tmp/net-same-zone-measure.sh $SRV_NAT $PARALLEL outside-vpc" | tee "$RESULTS_OUTSIDE"

if [ -n "${NAPKIN_CSV:-}" ]; then
  # shellcheck source=scripts/net-probe-csv.sh
  source "$REPO_DIR/scripts/net-probe-csv.sh"
  net_probe_append_csv "$RESULTS_INSIDE" "network_inside_vpc"
  net_probe_append_csv "$RESULTS_OUTSIDE" "network_outside_vpc"
fi

echo "=== GCP same-zone probe: OK ==="
