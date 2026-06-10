#!/usr/bin/env bash
# One AWS EC2 instance: create -> run `daily` -> append CSV -> terminate.
# Called per-instance by bench-fleet-aws-ci.sh. Env in: AWS_REGION, REPO_DIR, COMMIT, TIMESTAMP.
# Requires the napkin-bench key pair + napkin-bench-ssh security group (machines/aws-setup.sh).
set -euo pipefail

INSTANCE_TYPE="${1:?Usage: bench-worker-aws.sh <instance-type>}"
REGION="${AWS_REGION:-us-east-2}"
KEY_NAME="napkin-bench"
SG_NAME="napkin-bench-ssh"
KEY_FILE="${AWS_KEY_FILE:-$HOME/.ssh/napkin-bench.pem}"
REPO_DIR="${REPO_DIR:-.}"
CSV="$REPO_DIR/data/results.csv"
NAME="bench-${INSTANCE_TYPE//\./-}"

# Graviton (c7g) is ARM; everything else x86_64.
FAMILY="${INSTANCE_TYPE%%.*}"
if [[ "$FAMILY" == *g && "$FAMILY" != *gn ]]; then ARCH="arm64"; else ARCH="amd64"; fi

AMI_ID=$(aws ec2 describe-images --region "$REGION" --owners 099720109477 \
  --filters "Name=name,Values=ubuntu/images/hvm-ssd-gp3/ubuntu-noble-24.04-${ARCH}-server-*" \
            "Name=state,Values=available" \
  --query 'sort_by(Images, &CreationDate)[-1].ImageId' --output text)
SG_ID=$(aws ec2 describe-security-groups --region "$REGION" \
  --group-names "$SG_NAME" --query 'SecurityGroups[0].GroupId' --output text)

cleanup() {
  [ -n "${INSTANCE_ID:-}" ] && aws ec2 terminate-instances --region "$REGION" \
    --instance-ids "$INSTANCE_ID" >/dev/null 2>&1 || true
}
trap cleanup EXIT

echo "=== $NAME ($INSTANCE_TYPE, $ARCH) ==="
INSTANCE_ID=$(aws ec2 run-instances --region "$REGION" \
  --instance-type "$INSTANCE_TYPE" --image-id "$AMI_ID" --key-name "$KEY_NAME" \
  --security-group-ids "$SG_ID" \
  --block-device-mappings 'DeviceName=/dev/sda1,Ebs={VolumeSize=50,VolumeType=gp3}' \
  --tag-specifications "ResourceType=instance,Tags=[{Key=Name,Value=$NAME}]" \
  --query 'Instances[0].InstanceId' --output text)
aws ec2 wait instance-running --region "$REGION" --instance-ids "$INSTANCE_ID"
PUBLIC_IP=$(aws ec2 describe-instances --region "$REGION" --instance-ids "$INSTANCE_ID" \
  --query 'Reservations[0].Instances[0].PublicIpAddress' --output text)

SSH="ssh -i $KEY_FILE -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR ubuntu@$PUBLIC_IP"
SCP="scp -i $KEY_FILE -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR"
for _ in $(seq 1 30); do $SSH "echo ready" 2>/dev/null && break; sleep 5; done

tar czf "/tmp/${NAME}.tar.gz" -C "$REPO_DIR" --exclude=target --exclude=.git --exclude=data --exclude=SIMON.md .
$SCP "/tmp/${NAME}.tar.gz" "ubuntu@$PUBLIC_IP:~/repo.tar.gz"
rm -f "/tmp/${NAME}.tar.gz"
$SSH "mkdir -p ~/napkin-math-dreams/data && tar xzf ~/repo.tar.gz -C ~/napkin-math-dreams"

$SSH "
  sudo apt-get update -qq
  sudo apt-get install -y -qq build-essential pkg-config libssl-dev
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source ~/.cargo/env
  cd napkin-math-dreams && cargo build --release --bin daily
"
$SSH "cd napkin-math-dreams && sudo bash tuning/bench_stable.sh" || true
$SSH "
  source ~/.cargo/env
  cd napkin-math-dreams
  NAPKIN_MACHINE=aws-$INSTANCE_TYPE NAPKIN_CONFIG=bench_stable \
    NAPKIN_COMMIT='${COMMIT:-}' NAPKIN_TIMESTAMP='${TIMESTAMP:-}' NAPKIN_CSV=data/results.csv \
    cargo run --release --bin daily
"
$SCP "ubuntu@$PUBLIC_IP:~/napkin-math-dreams/data/results.csv" "/tmp/${NAME}.csv"

# Append into shared results.csv (skip header); lockfile so parallel workers don't interleave.
LOCK="$REPO_DIR/data/.results.lock"
while ! mkdir "$LOCK" 2>/dev/null; do sleep 0.5; done
tail -n +2 "/tmp/${NAME}.csv" >> "$CSV"
rmdir "$LOCK"
rm -f "/tmp/${NAME}.csv"
echo "=== $NAME done ==="
