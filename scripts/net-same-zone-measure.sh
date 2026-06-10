#!/usr/bin/env bash
# Run on the client VM. Server must have iperf3 -s listening on 5201.
# Usage: net-same-zone-measure.sh <server-internal-ip> [parallel-streams]
# iperf3 -J already reports throughput AND the kernel's RTT (TCP_INFO) in one run,
# so there's nothing else to install or coordinate.
set -euo pipefail

SRV_IP="${1:?server internal IP}"
PARALLEL="${2:-4}"
DURATION="${NET_PROBE_DURATION:-10}"

echo "=== network same-zone: iperf3 -P ${PARALLEL} (throughput + RTT) ==="
iperf3 -c "$SRV_IP" -t "$DURATION" -P "$PARALLEL" -J | tee /tmp/iperf3-net-same-zone.json

python3 - /tmp/iperf3-net-same-zone.json <<'PY'
import json, sys
end = json.load(open(sys.argv[1]))["end"]
print(f"napkin_throughput_gbit_s={end['sum_received']['bits_per_second'] / 1e9:.4f}")
# iperf3 reports the kernel's smoothed RTT per stream, in microseconds.
rtts = [s["sender"]["rtt"] for s in end["streams"] if s.get("sender", {}).get("rtt")]
if rtts:
    print(f"napkin_rtt_p50_ns={int(sum(rtts) / len(rtts) * 1000)}")
PY
