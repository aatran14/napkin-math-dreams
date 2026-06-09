#!/usr/bin/env bash
# Append network_same_zone rows to NAPKIN_CSV from measure output on stdout/file.
net_same_zone_append_csv() {
  local results_file="${1:?results file}"
  local csv="${NAPKIN_CSV:-}"
  [ -n "$csv" ] || return 0

  local gbit p50 p99
  gbit=$(grep '^napkin_throughput_gbit_s=' "$results_file" | tail -1 | cut -d= -f2)
  p50=$(grep '^napkin_rtt_p50_ns=' "$results_file" | tail -1 | cut -d= -f2)
  p99=$(grep '^napkin_rtt_p99_ns=' "$results_file" | tail -1 | cut -d= -f2)

  local date="${NAPKIN_TIMESTAMP:-$(date -u +%Y-%m-%dT%H:%M:%SZ)}"
  local machine="${NAPKIN_MACHINE:?NAPKIN_MACHINE required when NAPKIN_CSV is set}"
  local cpu="${NAPKIN_CPU:-}"
  local config="${NAPKIN_CONFIG:-bench_stable}"
  local commit="${NAPKIN_COMMIT:-}"

  local thr_bytes=""
  if [ -n "$gbit" ]; then
    thr_bytes=$(python3 -c "print(int(float('${gbit}') * 125000000))")
  fi

  local lock="${csv}.lock"
  while ! mkdir "$lock" 2>/dev/null; do sleep 0.5; done
  if [ ! -s "$csv" ]; then
    echo 'date,machine,cpu,config,operation,latency_ns,throughput_bytes_s,commit' >> "$csv"
  fi
  if [ -n "$thr_bytes" ]; then
    echo "${date},${machine},${cpu},${config},network_same_zone,,${thr_bytes},${commit}" >> "$csv"
  fi
  if [ -n "$p50" ]; then
    echo "${date},${machine},${cpu},${config},network_same_zone_rtt,${p50},,${commit}" >> "$csv"
  fi
  if [ -n "$p99" ]; then
    echo "${date},${machine},${cpu},${config},network_same_zone_rtt_p99,${p99},,${commit}" >> "$csv"
  fi
  rmdir "$lock"
}
