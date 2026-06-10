#!/usr/bin/env bash
# bench_stable: remove noise for stable, reproducible measurements.
# Derived from Simon's ./run script and easyperf.net recommendations.
# Must tolerate missing sysfs paths (GCP ARM VMs, etc).
set -e

echo "config: bench_stable"

read_knob() {
  local path="$1"
  if [ -f "$path" ]; then
    tr -d ' \n' < "$path"
  fi
}

report_knob() {
  local name="$1"
  local want="$2"
  local got="$3"
  if [ -z "$got" ]; then
    echo "  $name: skipped (not available)"
  elif [ "$got" = "$want" ]; then
    echo "  $name: applied"
  else
    echo "  $name: skipped (want=$want got=$got)"
  fi
}

# --- CPU frequency: lock to max, no dynamic scaling ---
if compgen -G "/sys/devices/system/cpu/cpu*/cpufreq/scaling_governor" > /dev/null; then
  for gov in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do
    [ -f "$gov" ] && echo "performance" | sudo tee "$gov" > /dev/null
  done
  report_knob "cpufreq governor" "performance" "$(read_knob /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor)"
else
  report_knob "cpufreq governor" "performance" ""
fi

# --- Turbo boost off ---
if [ -f /sys/devices/system/cpu/intel_pstate/no_turbo ]; then
  echo 1 | sudo tee /sys/devices/system/cpu/intel_pstate/no_turbo > /dev/null
  report_knob "turbo boost (intel)" "1" "$(read_knob /sys/devices/system/cpu/intel_pstate/no_turbo)"
elif [ -f /sys/devices/system/cpu/cpufreq/boost ]; then
  echo 0 | sudo tee /sys/devices/system/cpu/cpufreq/boost > /dev/null
  report_knob "turbo boost (amd)" "0" "$(read_knob /sys/devices/system/cpu/cpufreq/boost)"
else
  report_knob "turbo boost" "off" ""
fi

# --- Disable hyperthreading ---
if [ -x script/toggle-hyperthreading ] && command -v ruby > /dev/null 2>&1; then
  sudo script/toggle-hyperthreading -d > /dev/null 2>&1 || true
fi
if [ -f /sys/devices/system/cpu/cpu0/topology/thread_siblings_list ]; then
  physical=$(
    for sibs_file in /sys/devices/system/cpu/cpu*/topology/thread_siblings_list; do
      read_knob "$sibs_file"
    done | sort -u | wc -l | tr -d ' '
  )
  logical_online=0
  for online_file in /sys/devices/system/cpu/cpu*/online; do
    [ "$(read_knob "$online_file")" = "1" ] && logical_online=$((logical_online + 1))
  done
  if [ "$logical_online" -eq "$physical" ]; then
    echo "  hyperthreading: applied (off, ${physical} cores online)"
  else
    echo "  hyperthreading: skipped (want off, ${logical_online} logical / ${physical} physical cores online)"
  fi
else
  report_knob "hyperthreading" "off" ""
fi

# --- Drop filesystem caches ---
sync
echo 3 | sudo tee /proc/sys/vm/drop_caches > /dev/null
echo "  caches: applied (drop requested)"

# --- Disable ASLR ---
echo 0 | sudo tee /proc/sys/kernel/randomize_va_space > /dev/null
report_knob "ASLR" "0" "$(read_knob /proc/sys/kernel/randomize_va_space)"

# --- Transparent hugepages ---
if [ -f /sys/kernel/mm/transparent_hugepage/enabled ]; then
  echo 'always' | sudo tee /sys/kernel/mm/transparent_hugepage/enabled > /dev/null
  echo 'always' | sudo tee /sys/kernel/mm/transparent_hugepage/defrag > /dev/null
  thp_enabled=$(read_knob /sys/kernel/mm/transparent_hugepage/enabled)
  if [[ "$thp_enabled" == *"[always]"* ]]; then
    echo "  THP: applied"
  else
    echo "  THP: skipped (want=[always] got=$thp_enabled)"
  fi
else
  report_knob "THP" "always" ""
fi

# --- Allow perf events ---
if sudo sysctl -w kernel.perf_event_paranoid=-1 > /dev/null 2>&1; then
  report_knob "perf_event_paranoid" "-1" "$(sysctl -n kernel.perf_event_paranoid 2>/dev/null || true)"
else
  report_knob "perf_event_paranoid" "-1" ""
fi

echo "done"
