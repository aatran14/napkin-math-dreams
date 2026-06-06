#!/usr/bin/env bash
# One-time AWS setup: create SSH key pair and security group for benchmarking.
# Run this once before using bench-aws.sh.
set -euo pipefail

REGION="${AWS_REGION:-us-east-2}"
KEY_NAME="napkin-bench"
SG_NAME="napkin-bench-ssh"

echo "=== creating key pair '$KEY_NAME' ==="
if aws ec2 describe-key-pairs --region "$REGION" --key-names "$KEY_NAME" &>/dev/null; then
  echo "key pair already exists, skipping"
else
  aws ec2 create-key-pair --region "$REGION" \
    --key-name "$KEY_NAME" \
    --query 'KeyMaterial' --output text > ~/.ssh/napkin-bench.pem
  chmod 600 ~/.ssh/napkin-bench.pem
  echo "saved to ~/.ssh/napkin-bench.pem"
fi

echo "=== creating security group '$SG_NAME' ==="
VPC_ID=$(aws ec2 describe-vpcs --region "$REGION" --filters "Name=isDefault,Values=true" --query 'Vpcs[0].VpcId' --output text)
if aws ec2 describe-security-groups --region "$REGION" --group-names "$SG_NAME" &>/dev/null; then
  SG_ID=$(aws ec2 describe-security-groups --region "$REGION" --group-names "$SG_NAME" --query 'SecurityGroups[0].GroupId' --output text)
  echo "security group already exists: $SG_ID"
else
  SG_ID=$(aws ec2 create-security-group --region "$REGION" \
    --group-name "$SG_NAME" \
    --description "SSH access for napkin-math benchmarks" \
    --vpc-id "$VPC_ID" \
    --query 'GroupId' --output text)
  aws ec2 authorize-security-group-ingress --region "$REGION" \
    --group-id "$SG_ID" \
    --protocol tcp --port 22 --cidr 0.0.0.0/0
  echo "created: $SG_ID"
fi

echo ""
echo "=== done ==="
echo "key pair:       $KEY_NAME (~/.ssh/napkin-bench.pem)"
echo "security group: $SG_NAME ($SG_ID)"
echo ""
echo "you can now run: ./machines/bench-aws.sh c7i.4xlarge"
