#!/usr/bin/env bash
# Same-zone network: two matched AWS instances, iperf3 throughput + TCP RTT.
# Requires: ./machines/aws-setup.sh (SSH + iperf3/tcp/5201 within SG)
# Usage: ./machines/probe-net-pair-aws.sh [instance-type]
# Fleet: NAPKIN_CSV set → append network_same_zone rows to dead.csv
set -euo pipefail

INSTANCE_TYPE="${1:-t3.medium}"
REGION="${AWS_REGION:-us-east-2}"
AZ="${AWS_AZ:-${REGION}a}"
KEY_NAME="napkin-bench"
SG_NAME="napkin-bench-ssh"
KEY="${AWS_KEY_FILE:-$HOME/.ssh/napkin-bench.pem}"
REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
SUFFIX="${NAPKIN_RUN_SUFFIX:-$$}"
RESULTS="/tmp/net-same-zone-${SUFFIX}.out"

FAMILY="${INSTANCE_TYPE%%.*}"
if [[ "$FAMILY" == *g && "$FAMILY" != *gn ]]; then
  ARCH="arm64"
else
  ARCH="amd64"
fi

SG_ID=$(aws ec2 describe-security-groups --region "$REGION" \
  --group-names "$SG_NAME" --query 'SecurityGroups[0].GroupId' --output text)

# Ensure iperf3 (tcp/5201) is allowed within the SG so the pair can talk (idempotent).
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

echo "=== creating net-probe pair ($INSTANCE_TYPE) in $AZ ==="
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

for pub in "$SRV_PUB" "$CLI_PUB"; do
  ssh -i "$KEY" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR ubuntu@"$pub" \
    "sudo apt-get update -qq && sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq iperf3 python3"
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
ssh -i "$KEY" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR ubuntu@"$CLI_PUB" \
  "bash /tmp/net-same-zone-measure.sh $SRV_IP $PARALLEL" | tee "$RESULTS"

if [ -n "${NAPKIN_CSV:-}" ]; then
  # shellcheck source=scripts/net-same-zone-csv.sh
  source "$REPO_DIR/scripts/net-same-zone-csv.sh"
  net_same_zone_append_csv "$RESULTS"
fi

echo "=== AWS same-zone probe: OK ==="
