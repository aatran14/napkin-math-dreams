#!/usr/bin/env bash
# Spin up an AWS EC2 instance, build, run a binary, and tear down.
#
# Usage:
#   ./machines/run-aws.sh memory
#   ./machines/run-aws.sh memory c7i.4xlarge
#   ./machines/run-aws.sh readme c7g.4xlarge       # ARM
#
# Requires: aws CLI configured, aws-setup.sh already run
set -euo pipefail

BIN="${1:?Usage: run-aws.sh <binary-name> [instance-type]}"
INSTANCE_TYPE="${2:-c7i.xlarge}"
REGION="${AWS_REGION:-us-east-2}"
KEY_NAME="napkin-bench"
SG_NAME="napkin-bench-ssh"
REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
NAME="run-${BIN//_/-}"

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

echo "=== waiting for instance ==="
aws ec2 wait instance-running --region "$REGION" --instance-ids "$INSTANCE_ID"

PUBLIC_IP=$(aws ec2 describe-instances --region "$REGION" \
  --instance-ids "$INSTANCE_ID" \
  --query 'Reservations[0].Instances[0].PublicIpAddress' --output text)

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
  cargo build --release --bin $BIN
"

echo "=== running $BIN ==="
$SSH "
  source ~/.cargo/env
  cd napkin-math-dreams
  ./target/release/$BIN
"

echo ""
echo "=== done ==="
