## Running benchmarks

These run on cloud VMs. You create one through your provider's CLI, SSH in,
run the benchmarks, pull the CSV, and destroy the VM.

GCP example using a c4-standard-48-lssd:

```bash
# 1. create
gcloud compute instances create napkin-bench \
  --zone=us-central1-c \
  --machine-type=c4-standard-48-lssd \
  --image-family=ubuntu-2204-lts \
  --image-project=ubuntu-os-cloud

# 2. ssh
gcloud compute ssh napkin-bench --zone=us-central1-c

# 3. install
sudo apt-get update && sudo apt-get install -y build-essential pkg-config libssl-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env
git clone https://github.com/sirupsen/napkin-math && cd napkin-math
cargo build --release --bin daily

# 4. tune
sudo ./tuning/bench_stable.sh

# 5. run
NAPKIN_MACHINE=c4-standard-48-lssd NAPKIN_CONFIG=bench_stable cargo run --release --bin daily

# 6. pull results (from your local machine)
gcloud compute scp napkin-bench:~/napkin-math/data/dead.csv data/dead.csv --zone=us-central1-c

# 7. teardown
sudo ./tuning/teardown.sh

# 8. destroy
gcloud compute instances delete napkin-bench --zone=us-central1-c
```

Different machine? Change `--machine-type` and `NAPKIN_MACHINE`. Everything else is the same.
