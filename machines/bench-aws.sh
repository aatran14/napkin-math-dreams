#!/usr/bin/env bash
# Provision an AWS EC2 instance, run benchmarks, pull results, and tear down.
#
# Usage:
#   ./machines/bench-aws.sh c7i.12xlarge       # Intel Sapphire Rapids, 48 vCPU
#   ./machines/bench-aws.sh c7a.12xlarge       # AMD Genoa, 48 vCPU
#   ./machines/bench-aws.sh c7g.12xlarge       # Graviton3 ARM, 48 vCPU
#
# Requires: aws CLI configured, aws-setup.sh already run
set -euo pipefail

INSTANCE_TYPE="${1:?Usage: bench-aws.sh <instance-type>}"
REGION="${AWS_REGION:-us-east-2}"
KEY_NAME="napkin-bench"
SG_NAME="napkin-bench-ssh"
REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
NAME="bench-${INSTANCE_TYPE//\./-}"

# Pick AMI: ARM for Graviton, x86_64 otherwise
FAMILY="${INSTANCE_TYPE%%.*}"
if [[ "$FAMILY" == *g && "$FAMILY" != *gn ]]; then
  ARCH="arm64"
else
  ARCH="amd64"
fi

AMI_ID=$(aws ec2 describe-images --region "$REGION" \
  --owners 099720109477 \
  --filters "Name=name,Values=ubuntu/images/hvm-ssd-gp3/ubuntu-noble-24.04-${ARCH}-server-*" \
            "Name=state,Values=available" \
  --query 'sort_by(Images, &CreationDate)[-1].ImageId' --output text)

echo "=== AMI: $AMI_ID ($ARCH) ==="

SG_ID=$(aws ec2 describe-security-groups --region "$REGION" \
  --group-names "$SG_NAME" --query 'SecurityGroups[0].GroupId' --output text)

cleanup() {
  echo ""
  echo "=== cleaning up ==="
  if [ -n "${INSTANCE_ID:-}" ]; then
    aws ec2 terminate-instances --region "$REGION" --instance-ids "$INSTANCE_ID" >/dev/null || true
    echo "terminated $INSTANCE_ID"
  fi
}
trap cleanup EXIT

echo "=== creating $NAME ($INSTANCE_TYPE) in $REGION ==="
INSTANCE_ID=$(aws ec2 run-instances --region "$REGION" \
  --instance-type "$INSTANCE_TYPE" \
  --image-id "$AMI_ID" \
  --key-name "$KEY_NAME" \
  --security-group-ids "$SG_ID" \
  --block-device-mappings 'DeviceName=/dev/sda1,Ebs={VolumeSize=50,VolumeType=gp3}' \
  --tag-specifications "ResourceType=instance,Tags=[{Key=Name,Value=$NAME}]" \
  --query 'Instances[0].InstanceId' --output text)

echo "instance: $INSTANCE_ID"

echo "=== waiting for instance to be running ==="
aws ec2 wait instance-running --region "$REGION" --instance-ids "$INSTANCE_ID"

PUBLIC_IP=$(aws ec2 describe-instances --region "$REGION" \
  --instance-ids "$INSTANCE_ID" \
  --query 'Reservations[0].Instances[0].PublicIpAddress' --output text)

echo "IP: $PUBLIC_IP"

SSH="ssh -i ~/.ssh/napkin-bench.pem -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR ubuntu@$PUBLIC_IP"
SCP="scp -i ~/.ssh/napkin-bench.pem -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR"

echo "=== waiting for SSH ==="
for i in $(seq 1 30); do
  if $SSH "echo ready" 2>/dev/null; then
    break
  fi
  sleep 5
done

echo "=== installing deps ==="
$SSH "
  sudo apt-get update -qq
  sudo apt-get install -y -qq build-essential pkg-config libssl-dev
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
"

echo "=== uploading repo ==="
tar czf /tmp/${NAME}-repo.tar.gz -C "$REPO_DIR" --exclude=target --exclude=.git --exclude=SIMON.md --exclude=data .
$SCP /tmp/${NAME}-repo.tar.gz "ubuntu@$PUBLIC_IP:~/repo.tar.gz"
rm -f /tmp/${NAME}-repo.tar.gz
$SSH "mkdir -p ~/napkin-math-dreams/data && tar xzf ~/repo.tar.gz -C ~/napkin-math-dreams"

echo "=== building ==="
$SSH "
  source ~/.cargo/env
  cd napkin-math-dreams
  cargo build --release --bin daily
"

echo "=== tuning for stable benchmarks ==="
$SSH "
  cd napkin-math-dreams
  sudo bash tuning/bench_stable.sh
"

echo "=== running benchmarks ==="
$SSH "
  source ~/.cargo/env
  cd napkin-math-dreams
  NAPKIN_MACHINE=aws-$INSTANCE_TYPE NAPKIN_CONFIG=bench_stable cargo run --release --bin daily
"

echo "=== pulling results ==="
$SCP "ubuntu@$PUBLIC_IP:~/napkin-math-dreams/data/dead.csv" "/tmp/${NAME}.csv"

tail -n +2 "/tmp/${NAME}.csv" >> "$REPO_DIR/data/dead.csv"
rm -f "/tmp/${NAME}.csv"

echo ""
echo "=== $NAME done! results merged into data/dead.csv ==="
