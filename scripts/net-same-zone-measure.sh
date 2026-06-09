#!/usr/bin/env bash
# Run on the client VM. Server must have iperf3 -s listening on 5201.
# Usage: net-same-zone-measure.sh <server-internal-ip> [parallel-streams]
set -euo pipefail

SRV_IP="${1:?server internal IP}"
PARALLEL="${2:-4}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DURATION="${NET_PROBE_DURATION:-10}"
RTT_SAMPLES="${NET_RTT_SAMPLES:-1000}"

echo "=== network same-zone: throughput (iperf3 -P ${PARALLEL}) ==="
iperf3 -c "$SRV_IP" -t "$DURATION" -P "$PARALLEL" | tee /tmp/iperf3-net-same-zone.out

receiver_line=$(grep -E 'receiver|\[SUM\].*receiver' /tmp/iperf3-net-same-zone.out | tail -1 || true)
receiver_gbit=""
if [ -n "$receiver_line" ]; then
  receiver_gbit=$(echo "$receiver_line" | awk '{print $(NF-1)}')
  receiver_unit=$(echo "$receiver_line" | awk '{print $NF}')
  echo "receiver_rate=${receiver_gbit} ${receiver_unit}"
  echo "napkin_throughput_gbit_s=${receiver_gbit}"
fi

echo "=== network same-zone: latency (TCP connect RTT) ==="
python3 "$SCRIPT_DIR/net-same-zone-rtt.py" "$SRV_IP" 5201 "$RTT_SAMPLES" | tee /tmp/net-same-zone-rtt.out
p50_ns=$(grep '^p50_ns=' /tmp/net-same-zone-rtt.out | cut -d= -f2)
p99_ns=$(grep '^p99_ns=' /tmp/net-same-zone-rtt.out | cut -d= -f2)
[ -n "$p50_ns" ] && echo "napkin_rtt_p50_ns=${p50_ns}"
[ -n "$p99_ns" ] && echo "napkin_rtt_p99_ns=${p99_ns}"
