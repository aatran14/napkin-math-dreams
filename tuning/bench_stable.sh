#!/usr/bin/env bash
# bench_stable: remove noise for stable, reproducible measurements.
# Derived from Simon's ./run script and easyperf.net recommendations.
# Must tolerate missing sysfs paths (GCP ARM VMs, etc).
set -e

echo "config: bench_stable"

# --- CPU frequency: lock to max, no dynamic scaling ---
for gov in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do
  [ -f "$gov" ] && echo "performance" | sudo tee "$gov" > /dev/null
done

# --- Turbo boost off ---
# Intel
if [ -f /sys/devices/system/cpu/intel_pstate/no_turbo ]; then
  echo 1 | sudo tee /sys/devices/system/cpu/intel_pstate/no_turbo > /dev/null
  echo "  turbo boost: off (intel)"
fi
# AMD
if [ -f /sys/devices/system/cpu/cpufreq/boost ]; then
  echo 0 | sudo tee /sys/devices/system/cpu/cpufreq/boost > /dev/null
  echo "  turbo boost: off (amd)"
fi

# --- Disable hyperthreading ---
# Skip if Ruby not installed or script not present
if [ -x script/toggle-hyperthreading ] && command -v ruby > /dev/null 2>&1; then
  sudo script/toggle-hyperthreading -d > /dev/null
  echo "  hyperthreading: off"
fi

# --- Drop filesystem caches ---
sync
echo 3 | sudo tee /proc/sys/vm/drop_caches > /dev/null
echo "  caches: dropped"

# --- Disable ASLR ---
echo 0 | sudo tee /proc/sys/kernel/randomize_va_space > /dev/null
echo "  ASLR: off"

# --- Transparent hugepages ---
if [ -f /sys/kernel/mm/transparent_hugepage/enabled ]; then
  echo 'always' | sudo tee /sys/kernel/mm/transparent_hugepage/enabled > /dev/null
  echo 'always' | sudo tee /sys/kernel/mm/transparent_hugepage/defrag > /dev/null
  echo "  THP: always"
fi

# --- Allow perf events ---
sudo sysctl -w kernel.perf_event_paranoid=-1 > /dev/null 2>&1 || true

echo "done"
