#!/usr/bin/env bash
# Provision an AWS EC2 instance, run benchmarks, pull results, and tear down.
#
# Usage:
#   ./machines/bench-aws.sh c7i.4xlarge        # Intel, 16 vCPU
#   ./machines/bench-aws.sh c7a.4xlarge        # AMD, 16 vCPU
#   ./machines/bench-aws.sh c7g.4xlarge        # Graviton3 ARM, 16 vCPU
#
# Requires: aws CLI configured, aws-setup.sh already run
set -euo pipefail

INSTANCE_TYPE="${1:?Usage: bench-aws.sh <instance-type>}"
REGION="${AWS_REGION:-us-east-2}"
KEY_NAME="napkin-bench"
SG_NAME="napkin-bench-ssh"
S3_BUCKET="${NAPKIN_S3_BUCKET:-napkin-math-bench-${REGION}}"
S3_PROFILE="${NAPKIN_S3_INSTANCE_PROFILE:-napkin-bench-s3}"
REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
NAME="bench-${INSTANCE_TYPE//\./-}"

# Pick AMI: ARM for Graviton, x86_64 otherwise
FAMILY="${INSTANCE_TYPE%%.*}"
if [[ "$FAMILY" == *g && "$FAMILY" != *gn ]]; then
  ARCH="arm64"
else
  ARCH="amd64"
fi

BIN_DIR="${NAPKIN_BIN_DIR:-$REPO_DIR/bin}"
BIN="$BIN_DIR/daily-$ARCH/daily"
if [ ! -f "$BIN" ]; then
  echo "missing prebuilt binary: $BIN" >&2
  echo "build it with: cargo build --release --bin daily   (on a linux/$ARCH host)" >&2
  exit 1
fi

AMI_ID=$(aws ec2 describe-images --region "$REGION" \
  --owners 099720109477 \
  --filters "Name=name,Values=ubuntu/images/hvm-ssd-gp3/ubuntu-noble-24.04-${ARCH}-server-*" \
            "Name=state,Values=available" \
  --query 'sort_by(Images, &CreationDate)[-1].ImageId' --output text)

echo "=== $NAME ($INSTANCE_TYPE) ==="
echo "AMI: $AMI_ID ($ARCH)"

SG_ID=$(aws ec2 describe-security-groups --region "$REGION" \
  --group-names "$SG_NAME" --query 'SecurityGroups[0].GroupId' --output text)

cleanup() {
  echo ""
  echo "=== tearing down $NAME ==="
  if [ -n "${INSTANCE_ID:-}" ]; then
    aws ec2 terminate-instances --region "$REGION" --instance-ids "$INSTANCE_ID" >/dev/null || true
    echo "terminated $INSTANCE_ID"
  fi
}
trap cleanup EXIT

echo "provisioning VM in $REGION..."
IAM_ARGS=()
if aws iam get-instance-profile --instance-profile-name "$S3_PROFILE" &>/dev/null; then
  IAM_ARGS=(--iam-instance-profile "Name=${S3_PROFILE}")
  echo "S3 instance profile: $S3_PROFILE"
else
  echo "warning: no instance profile $S3_PROFILE — run machines/aws-s3-setup.sh for s3_get"
fi
run_instance() {
  aws ec2 run-instances --region "$REGION" \
    --instance-type "$INSTANCE_TYPE" \
    --image-id "$AMI_ID" \
    --key-name "$KEY_NAME" \
    --security-group-ids "$SG_ID" \
    "${IAM_ARGS[@]}" \
    --block-device-mappings 'DeviceName=/dev/sda1,Ebs={VolumeSize=50,VolumeType=gp3}' \
    --tag-specifications "ResourceType=instance,Tags=[{Key=Name,Value=$NAME}]" \
    --query 'Instances[0].InstanceId' --output text "$@"
}

# EC2 has no --max-run-duration, so the instance carries its own kill switch: it
# halts after 3h and halting terminates it, which survives the runner being
# cancelled. Same rule as GCP though — a safety net must never be the reason a
# run doesn't start, so fall back to a plain instance if EC2 rejects it.
if ! INSTANCE_ID=$(run_instance \
      --instance-initiated-shutdown-behavior terminate \
      --user-data "$(printf '#!/bin/bash\nshutdown -h +180\n')"); then
  echo "auto-shutoff not accepted here, provisioning without it"
  INSTANCE_ID=$(run_instance)
fi

echo "instance: $INSTANCE_ID"

echo "waiting for instance to be running..."
aws ec2 wait instance-running --region "$REGION" --instance-ids "$INSTANCE_ID"

PUBLIC_IP=$(aws ec2 describe-instances --region "$REGION" \
  --instance-ids "$INSTANCE_ID" \
  --query 'Reservations[0].Instances[0].PublicIpAddress' --output text)

echo "IP: $PUBLIC_IP"

SSH="ssh -i ~/.ssh/napkin-bench.pem -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR ubuntu@$PUBLIC_IP"
SCP="scp -i ~/.ssh/napkin-bench.pem -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR"

echo "waiting for SSH..."
for i in $(seq 1 30); do
  if $SSH "echo ready" 2>/dev/null; then
    break
  fi
  sleep 5
done

# daily reads the benchmarks/ TOML tree at runtime to decide what to run and
# panics if it isn't there, so it ships with the binary. tuning/ is the
# bench_stable.sh the VM runs before measuring. That's the whole payload.
echo "uploading binary and benchmark definitions..."
tar czf "/tmp/${NAME}-payload.tar.gz" -C "$REPO_DIR" benchmarks tuning
$SCP "/tmp/${NAME}-payload.tar.gz" "ubuntu@$PUBLIC_IP:~/payload.tar.gz"
$SCP "$BIN" "ubuntu@$PUBLIC_IP:~/daily"
rm -f "/tmp/${NAME}-payload.tar.gz"
$SSH "
  set -e
  mkdir -p ~/napkin/data
  tar xzf ~/payload.tar.gz -C ~/napkin
  mv ~/daily ~/napkin/daily
  chmod +x ~/napkin/daily
  test -d ~/napkin/benchmarks
  test -f ~/napkin/tuning/bench_stable.sh
"

echo "tuning VM for stable benchmarks..."
$SSH "
  cd napkin
  sudo bash tuning/bench_stable.sh
"

echo "running benchmarks..."
$SSH "
  cd napkin
  NAPKIN_MACHINE=aws-$INSTANCE_TYPE NAPKIN_CONFIG=bench_stable \
    NAPKIN_S3_BUCKET=$S3_BUCKET ./daily
"

echo "pulling results..."
$SCP "ubuntu@$PUBLIC_IP:~/napkin/data/dead.csv" "/tmp/${NAME}.csv"

tail -n +2 "/tmp/${NAME}.csv" >> "$REPO_DIR/data/dead.csv"
rm -f "/tmp/${NAME}.csv"

echo ""
echo "=== $NAME done! results merged into data/dead.csv ==="
