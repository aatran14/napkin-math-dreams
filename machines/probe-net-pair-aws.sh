#!/usr/bin/env bash
# Same-zone network: two matched AWS instances, iperf3 inside + outside VPC.
# Inside:  client → server PrivateIpAddress
# Outside: client → server PublicIpAddress (hairpin through public path)
# Requires: ./machines/aws-setup.sh (SSH + iperf3/tcp/5201 within SG)
# Usage: ./machines/probe-net-pair-aws.sh [instance-type]
# Fleet: NAPKIN_CSV set → append network_inside_vpc* / network_outside_vpc* rows
set -euo pipefail

INSTANCE_TYPE="${1:-t3.medium}"
REGION="${AWS_REGION:-us-east-2}"
AZ="${AWS_AZ:-${REGION}a}"
KEY_NAME="napkin-bench"
SG_NAME="napkin-bench-ssh"
KEY="${AWS_KEY_FILE:-$HOME/.ssh/napkin-bench.pem}"
REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
SUFFIX="${NAPKIN_RUN_SUFFIX:-$$}"
RESULTS_INSIDE="/tmp/net-inside-vpc-${SUFFIX}.out"
RESULTS_OUTSIDE="/tmp/net-outside-vpc-${SUFFIX}.out"

FAMILY="${INSTANCE_TYPE%%.*}"
if [[ "$FAMILY" == *g && "$FAMILY" != *gn ]]; then
  ARCH="arm64"
else
  ARCH="amd64"
fi

SG_ID=$(aws ec2 describe-security-groups --region "$REGION" \
  --group-names "$SG_NAME" --query 'SecurityGroups[0].GroupId' --output text)

# iperf3 between pair members (private or public IP — same SG).
aws ec2 authorize-security-group-ingress --region "$REGION" \
  --group-id "$SG_ID" --protocol tcp --port 5201 --source-group "$SG_ID" >/dev/null 2>&1 || true

AMI_ID=$(aws ec2 describe-images --region "$REGION" \
  --owners 099720109477 \
  --filters "Name=name,Values=ubuntu/images/hvm-ssd-gp3/ubuntu-noble-24.04-${ARCH}-server-*" \
            "Name=state,Values=available" \
  --query 'sort_by(Images, &CreationDate)[-1].ImageId' --output text)

cleanup() {
  echo "=== tearing down ==="
  for id in ${SRV_ID:-} ${CLI_ID:-}; do
    [ -n "$id" ] && aws ec2 terminate-instances --region "$REGION" --instance-ids "$id" >/dev/null 2>&1 || true
  done
}
trap cleanup EXIT

echo "=== provisioning net-probe pair ($INSTANCE_TYPE) in $AZ ==="
SRV_ID=$(aws ec2 run-instances --region "$REGION" \
  --image-id "$AMI_ID" --instance-type "$INSTANCE_TYPE" --key-name "$KEY_NAME" \
  --security-group-ids "$SG_ID" --placement "AvailabilityZone=$AZ" \
  --tag-specifications "ResourceType=instance,Tags=[{Key=Name,Value=net-probe-srv-${SUFFIX}}]" \
  --query 'Instances[0].InstanceId' --output text)
CLI_ID=$(aws ec2 run-instances --region "$REGION" \
  --image-id "$AMI_ID" --instance-type "$INSTANCE_TYPE" --key-name "$KEY_NAME" \
  --security-group-ids "$SG_ID" --placement "AvailabilityZone=$AZ" \
  --tag-specifications "ResourceType=instance,Tags=[{Key=Name,Value=net-probe-cli-${SUFFIX}}]" \
  --query 'Instances[0].InstanceId' --output text)

aws ec2 wait instance-running --region "$REGION" --instance-ids "$SRV_ID" "$CLI_ID"
aws ec2 wait instance-status-ok --region "$REGION" --instance-ids "$SRV_ID" "$CLI_ID"

SRV_IP=$(aws ec2 describe-instances --region "$REGION" --instance-ids "$SRV_ID" \
  --query 'Reservations[0].Instances[0].PrivateIpAddress' --output text)
SRV_PUB=$(aws ec2 describe-instances --region "$REGION" --instance-ids "$SRV_ID" \
  --query 'Reservations[0].Instances[0].PublicIpAddress' --output text)
CLI_PUB=$(aws ec2 describe-instances --region "$REGION" --instance-ids "$CLI_ID" \
  --query 'Reservations[0].Instances[0].PublicIpAddress' --output text)

echo "=== server internal IP: $SRV_IP ==="
echo "=== server public IP:   $SRV_PUB ==="

if [ -z "$SRV_PUB" ]; then
  echo "error: probe instances need public IPs for outside-VPC path" >&2
  exit 1
fi

wait_ssh() {
  local pub=$1 label=$2
  for i in $(seq 1 48); do
    if ssh -i "$KEY" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
      -o LogLevel=ERROR -o ConnectTimeout=10 ubuntu@"$pub" "echo ready" 2>/dev/null; then
      echo "$label: ssh ok"
      return 0
    fi
    sleep 10
  done
  echo "$label: ssh timeout"
  return 1
}
wait_ssh "$SRV_PUB" srv
wait_ssh "$CLI_PUB" cli

# iperf3 is a system package, so unlike the compute benchmarks this pair can't
# be handed a prebuilt binary and must still talk to the Ubuntu mirror. Mirror
# 503s are what used to lose whole nights, and they pass in seconds, so retry
# instead of letting the first one end the probe.
echo "installing iperf3..."
for pub in "$SRV_PUB" "$CLI_PUB"; do
  ssh -i "$KEY" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR ubuntu@"$pub" '
    set -e
    for attempt in 1 2 3 4 5; do
      if sudo apt-get update -qq \
        && sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq iperf3 python3; then
        exit 0
      fi
      echo "apt attempt $attempt failed, retrying in $((attempt * 10))s"
      sleep $((attempt * 10))
    done
    echo "apt failed 5 times, giving up" >&2
    exit 1
  '
done

if [ -z "${NAPKIN_CPU:-}" ]; then
  NAPKIN_CPU=$(ssh -i "$KEY" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR ubuntu@"$CLI_PUB" \
    "lscpu 2>/dev/null | awk -F: '/Model name/{print \$2; exit}'" | sed 's/^[[:space:]]*//')
  export NAPKIN_CPU
fi

ssh -i "$KEY" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR ubuntu@"$SRV_PUB" \
  "pkill iperf3 2>/dev/null || true; nohup iperf3 -s > /tmp/iperf3-srv.log 2>&1 & sleep 2; pgrep -a iperf3"

scp -i "$KEY" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
  "$REPO_DIR/scripts/net-same-zone-measure.sh" \
  "ubuntu@${CLI_PUB}:/tmp/"

PARALLEL="${NET_PROBE_PARALLEL:-4}"

echo "=== inside VPC: client → $SRV_IP ==="
ssh -i "$KEY" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR ubuntu@"$CLI_PUB" \
  "bash /tmp/net-same-zone-measure.sh $SRV_IP $PARALLEL inside-vpc" | tee "$RESULTS_INSIDE"

echo "=== outside VPC: client → $SRV_PUB (hairpin) ==="
ssh -i "$KEY" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR ubuntu@"$CLI_PUB" \
  "bash /tmp/net-same-zone-measure.sh $SRV_PUB $PARALLEL outside-vpc" | tee "$RESULTS_OUTSIDE"

if [ -n "${NAPKIN_CSV:-}" ]; then
  # shellcheck source=scripts/net-probe-csv.sh
  source "$REPO_DIR/scripts/net-probe-csv.sh"
  net_probe_append_csv "$RESULTS_INSIDE" "network_inside_vpc"
  net_probe_append_csv "$RESULTS_OUTSIDE" "network_outside_vpc"
fi

echo "=== AWS same-zone probe: OK ==="
