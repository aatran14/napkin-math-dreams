# Latency Roadmap

Napkin Math currently has two benchmark styles:

- Criterion throughput benches in `benches/`.
- An older ad hoc harness in `src/main.rs` that reports averages over fixed time windows.

The next level is a third path: raw latency distributions. Throughput and
averages are still useful, but latency work needs retained samples, percentiles,
histograms, and enough machine metadata to compare runs over time.

## First Slice

`src/bin/latency.rs` is the starter sampler. It runs one probe many times,
prints summary percentiles, renders an ASCII histogram, and can write raw CSV:

```bash
./script/latency syscall_getpid --samples 100000
./script/latency tcp_echo --size 1 --samples 50000
./script/latency memory_random --size 256m --batch 1024
```

Each sample is the average latency of `--batch` operations. That matters for
nanosecond-scale probes where timer overhead is similar to the operation being
measured. For slower probes, keep `--batch 1` to preserve per-operation tails.

## Daily Runner

A daily run should be a matrix, not one host:

- Cloud: AWS, GCP, Azure, and bare metal when available.
- Families: general purpose, compute optimized, memory optimized, storage
  optimized, burstable, and Arm variants.
- Sizes: one small size for cost, one large size for saturation, and one
  representative production size.
- Regions and zones: at least one fixed primary region per provider, with
  occasional cross-region sweeps for network and blob probes.

Use a stable image and run from a path without spaces. The current macOS compile
issue with old `jemalloc-sys` is a good reminder that benchmark automation
should remove environmental weirdness before interpreting performance.

## Result Shape

Keep raw samples and summary data. A useful run record should include:

- `run_id`, UTC timestamp, git SHA, dirty flag, command, probe, size, batch.
- Provider, region, zone, instance type, vCPU count, memory, disk shape, NIC
  limit.
- CPU model, microcode, NUMA layout, SMT state, turbo state, governor, kernel,
  filesystem, I/O scheduler, transparent hugepage state, and relevant sysctls.
- Raw latency samples, summary percentiles, histogram bucket counts, and any
  perf counters collected with the run.

CSV is enough for the first local artifact. The durable format should probably
also include JSON metadata plus an HdrHistogram-compatible binary or text export.

## Kernel Tuning Loop

Treat tuning as explicit experiment variants:

- `baseline`: stock image with only required dependencies.
- `bench_stable`: performance governor, pinned cores, fixed SMT policy, fixed
  turbo policy, stable THP policy, warmed caches where appropriate.
- `probe_specific`: network socket buffers and IRQ affinity for network probes,
  I/O scheduler and queue depth for disk probes, NUMA memory binding for memory
  probes.

Every tuning change needs to be recorded as data. Avoid a silent magic setup
script: the interesting answer is often which knob moved which percentile.

## Analysis Loop

Daily reports should answer four questions:

- What changed since yesterday on the same machine type?
- How do the distributions differ across modern machine families?
- Which machines are closest to the expected hardware limit?
- Which tail latencies are kernel, runtime, device, or network artifacts?

For optimization, pair latency histograms with `perf stat`, `perf record`,
eBPF probes, flamegraphs, and provider metadata. The goal is not just "faster";
it is knowing whether a row is CPU-bound, memory-bound, syscall-bound,
device-bound, scheduler-bound, or provider-service-bound.

## Near-Term Backlog

- Move the useful probes from `src/main.rs` into reusable modules.
- Add HdrHistogram output for compact distribution storage.
- Add JSON metadata next to each CSV.
- Add a report generator that overlays histograms by machine type.
- Add a cloud runner that provisions one machine, runs the matrix, uploads
  artifacts, and tears the machine down.
- Add regression thresholds on p50, p95, p99, p99.9, and max.
- Add dedicated suites for blob first-byte latency, random disk reads, TCP echo,
  syscall families, mutex contention, and memory pointer chasing.
