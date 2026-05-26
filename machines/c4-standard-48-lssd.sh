#!/usr/bin/env bash
# Provision a GCP c4-standard-48-lssd instance for benchmarking.
# Intel Xeon 6985P-C, 48 vCPU / 24 physical cores, 180 GB RAM.
# This is the machine Simon uses for the README numbers.
set -e

ZONE="us-central1-c"
NAME="napkin-bench"
PROJECT="${NAPKIN_GCP_PROJECT:?set NAPKIN_GCP_PROJECT}"

gcloud compute instances create "$NAME" \
  --project="$PROJECT" \
  --zone="$ZONE" \
  --machine-type=c4-standard-48-lssd \
  --image-family=ubuntu-2204-lts \
  --image-project=ubuntu-os-cloud \
  --boot-disk-size=50GB \
  --boot-disk-type=pd-ssd

echo "waiting for SSH..."
gcloud compute ssh "$NAME" --zone="$ZONE" --project="$PROJECT" --command="echo ready"

echo "provisioning..."
gcloud compute ssh "$NAME" --zone="$ZONE" --project="$PROJECT" --command="
  sudo apt-get update -qq
  sudo apt-get install -y -qq build-essential pkg-config libssl-dev
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source ~/.cargo/env
  git clone https://github.com/sirupsen/napkin-math
  cd napkin-math
  cargo build --release --bin daily
"

echo ""
echo "instance ready. to run:"
echo "  gcloud compute ssh $NAME --zone=$ZONE --project=$PROJECT"
echo "  cd napkin-math"
echo "  sudo ./tuning/bench_stable.sh"
echo "  NAPKIN_MACHINE=c4-standard-48-lssd NAPKIN_CONFIG=bench_stable cargo run --release --bin daily"
echo "  sudo ./tuning/teardown.sh"
echo ""
echo "to destroy:"
echo "  gcloud compute instances delete $NAME --zone=$ZONE --project=$PROJECT"
