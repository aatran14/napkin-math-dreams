#!/usr/bin/env bash
# Restore safe defaults after a bench run.
# Simon's ./run script does this too.
set -e

echo "teardown: restoring defaults"

# Re-enable hyperthreading
if [ -x script/toggle-hyperthreading ]; then
  sudo script/toggle-hyperthreading -e > /dev/null
  echo "  hyperthreading: on"
fi

# Restore THP to madvise
echo 'madvise' | sudo tee /sys/kernel/mm/transparent_hugepage/enabled > /dev/null 2>&1 || true
echo 'madvise' | sudo tee /sys/kernel/mm/transparent_hugepage/defrag > /dev/null 2>&1 || true
echo "  THP: madvise"

# Re-enable ASLR
echo 2 | sudo tee /proc/sys/kernel/randomize_va_space > /dev/null 2>&1 || true
echo "  ASLR: on"

echo "done"
