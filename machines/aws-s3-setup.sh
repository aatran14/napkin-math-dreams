#!/usr/bin/env bash
# One-time: S3 bucket + bench object + EC2 instance profile for GET benchmarks.
# Usage: ./machines/aws-s3-setup.sh
set -euo pipefail

REGION="${AWS_REGION:-us-east-2}"
BUCKET="${NAPKIN_S3_BUCKET:-napkin-math-bench-${REGION}}"
KEY="${NAPKIN_S3_KEY:-bench-object}"
ROLE_NAME="napkin-bench-s3"
PROFILE_NAME="napkin-bench-s3"
OBJECT_SIZE=4096

echo "=== S3 bucket: s3://${BUCKET} (${REGION}) ==="
if aws s3api head-bucket --bucket "$BUCKET" 2>/dev/null; then
  echo "bucket already exists"
else
  if [ "$REGION" = "us-east-1" ]; then
    aws s3api create-bucket --bucket "$BUCKET" --region "$REGION"
  else
    aws s3api create-bucket --bucket "$BUCKET" --region "$REGION" \
      --create-bucket-configuration "LocationConstraint=${REGION}"
  fi
  echo "created bucket"
fi

TMP=$(mktemp)
dd if=/dev/urandom of="$TMP" bs="$OBJECT_SIZE" count=1 status=none
aws s3 cp "$TMP" "s3://${BUCKET}/${KEY}" --region "$REGION"
rm -f "$TMP"
echo "uploaded ${KEY} (${OBJECT_SIZE} bytes)"

TRUST='{"Version":"2012-10-17","Statement":[{"Effect":"Allow","Principal":{"Service":"ec2.amazonaws.com"},"Action":"sts:AssumeRole"}]}'
POLICY=$(cat <<EOF
{
  "Version": "2012-10-17",
  "Statement": [{
    "Effect": "Allow",
    "Action": ["s3:GetObject"],
    "Resource": "arn:aws:s3:::${BUCKET}/${KEY}"
  }]
}
EOF
)

echo "=== IAM role ${ROLE_NAME} ==="
if aws iam get-role --role-name "$ROLE_NAME" &>/dev/null; then
  echo "role already exists"
else
  aws iam create-role --role-name "$ROLE_NAME" --assume-role-policy-document "$TRUST" >/dev/null
  echo "created role"
fi
aws iam put-role-policy --role-name "$ROLE_NAME" --policy-name s3-get-bench \
  --policy-document "$POLICY"

if aws iam get-instance-profile --instance-profile-name "$PROFILE_NAME" &>/dev/null; then
  echo "instance profile already exists"
else
  aws iam create-instance-profile --instance-profile-name "$PROFILE_NAME" >/dev/null
  aws iam add-role-to-instance-profile --instance-profile-name "$PROFILE_NAME" \
    --role-name "$ROLE_NAME"
  echo "created instance profile"
fi

echo ""
echo "=== done ==="
echo "NAPKIN_S3_BUCKET=${BUCKET}"
echo "NAPKIN_S3_KEY=${KEY}"
echo "instance profile: ${PROFILE_NAME}"
echo ""
echo "bench-aws.sh attaches ${PROFILE_NAME} automatically."
