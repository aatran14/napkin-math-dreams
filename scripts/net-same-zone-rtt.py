#!/usr/bin/env python3
"""TCP connect RTT to host:port. ICMP/ping is often blocked on cloud SGs."""
import socket
import sys
import time


def percentile(sorted_samples, p):
    if not sorted_samples:
        return 0
    idx = int((len(sorted_samples) - 1) * p / 100)
    return sorted_samples[idx]


def main():
    if len(sys.argv) != 4:
        print(f"usage: {sys.argv[0]} <host> <port> <samples>", file=sys.stderr)
        sys.exit(2)

    host = sys.argv[1]
    port = int(sys.argv[2])
    samples = int(sys.argv[3])
    latencies = []

    for _ in range(samples):
        start = time.perf_counter_ns()
        sock = socket.create_connection((host, port), timeout=2)
        sock.close()
        latencies.append(time.perf_counter_ns() - start)

    latencies.sort()
    print(f"p50_ns={percentile(latencies, 50)}")
    print(f"p99_ns={percentile(latencies, 99)}")
    print(f"samples={len(latencies)}")


if __name__ == "__main__":
    main()
