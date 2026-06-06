# Memory Benchmark Isolation

The memory benchmarks want to measure DRAM. The benchmarking harness wants
multiple iterations for stable numbers. These two goals fight each other:
every iteration after the first reads from cache, not memory.

## The problem in detail

### Sequential read (single-threaded)

A 1 GiB buffer of u64s is read in a tight vectorized loop. The first pass
pulls every cache line from DRAM. The second pass finds all of it sitting in
L3 (or even L2 for the most recently touched regions). A 10-second measurement
window over a ~50 ms operation means ~200 iterations, of which ~199 measure
cache throughput.

On a machine with 36 MiB of L3, the first pass does 1 GiB / 64 bytes =
16 million cache-line fills from DRAM. Every subsequent pass does zero.

### Sequential read (threaded)

Same problem, worse arithmetic. The 1 GiB buffer is split evenly across all
cores. On a 48-core machine, each thread owns ~21 MiB — which fits inside
L3 entirely. Even the first pass may not measure DRAM if the allocator places
each thread's slice in local NUMA memory and the L3 is large enough.

The benchmark is supposed to saturate all memory channels. Instead it may be
saturating L3 bandwidth.

### Random read

A shuffled index array defeats the hardware prefetcher on the first pass.
But the `bench()` harness runs a 1-second warmup before measurement begins.
That warmup is enough to touch the entire 1 GiB buffer, pulling it into L3.
The measured 5-second window then reads warm cache in a random pattern —
which is slower than sequential cache reads (poor spatial locality within
cache lines) but still not DRAM latency.

Even without the warmup, the benchmark wraps around when it exhausts the
shuffled order. The second traversal reuses the same index sequence over
data that's now resident in L3.

## Why this is hard

The core tension:

1. **Stable numbers need many iterations.** A single pass over 1 GiB takes
   ~50-100 ms. Timer resolution is fine at that scale, but run-to-run variance
   from interrupts, frequency scaling, and NUMA effects is high. You want
   dozens of samples to build confidence.

2. **Cold memory needs few iterations.** To measure DRAM, each iteration must
   start with the data evicted from all levels of cache. But eviction is
   itself expensive and noisy, and it adds overhead that must be measured
   and subtracted.

3. **L3 size varies across the fleet.** A buffer that exceeds L3 on a laptop
   (12 MiB) fits comfortably on a C4 instance (50+ MiB L3). The buffer must
   be sized relative to the machine, not hardcoded.

4. **The harness was designed for throughput.** The `bench()` function and the
   hand-rolled loop in `seq_read_single` both assume the setup cost is paid
   once and the hot loop runs cleanly. That's right for hashing, sorting,
   syscalls — anything where repeated iteration doesn't change what you're
   measuring. Memory is different because the act of reading changes the
   state of the thing you're trying to measure.

## Possible approaches

### A. Flush between iterations

Use `clflushopt` (x86) or `dc civac` (ARM) to evict every cache line between
iterations. `verify_memory` already does this for a single validation pass.

**Pro:** keeps the existing multi-iteration structure. Each iteration is cold.

**Con:** flushing 1 GiB takes real time — millions of cache line evictions,
each of which is a serializing instruction. On x86, `clflushopt` is faster than
`clflush` but still ~5-10 ns per line. For 1 GiB that's ~80-160 ms of flush
overhead per iteration, comparable to the read itself. You'd need to measure
the flush separately and subtract it, which adds its own uncertainty. On some
microarchitectures the flush also writes back dirty lines, which pollutes
memory bus measurements.

### B. Single-pass, many runs

Run the benchmark once (single cold pass), record the result, and repeat
across many independent invocations. Statistical confidence comes from
aggregating runs, not iterations within a run.

**Pro:** each measurement is a true DRAM read. No eviction overhead. Clean
separation of concerns — the harness doesn't need to know about cache state.

**Con:** high per-run overhead (process startup, allocation, potentially
re-creating the shuffled index array). Variance is higher per sample, so you
need more runs to converge. Harder to fit into the current daily runner which
expects one invocation per benchmark.

### C. Buffer much larger than L3

Allocate a buffer that cannot fit in L3 on any target machine. If the largest
L3 in the fleet is 100 MiB, use a 1 GiB buffer (already the case for
single-threaded, but not for per-thread slices in threaded mode). On the second
pass, the beginning of the buffer has already been evicted by the end of the
first pass — the cache is a FIFO of sorts.

**Pro:** natural eviction without explicit flush instructions. Multiple
iterations work as long as the buffer is large enough relative to L3.

**Con:** "large enough" is a moving target as machines get bigger caches.
LRU eviction isn't perfectly FIFO — set-associativity means some lines
survive longer than expected. On NUMA machines, remote-node access patterns
change depending on the allocator. Also, the random read benchmark can't
benefit from this: random access means the most recently touched lines aren't
concentrated at the start of the buffer.

### D. Pointer chasing

Replace array indexing with a linked-list chase: each cache line contains
the address of the next cache line to read. The order is randomized at
setup time. The hardware prefetcher cannot predict the next address because
it depends on a load that hasn't completed.

**Pro:** defeats the prefetcher fundamentally, not just on the first pass.
Every load is a dependent load, so you measure true DRAM latency per access.
This is the standard technique (cf. lmbench lat_mem_rd, Intel MLC).

**Con:** measures latency, not bandwidth. Sequential throughput benchmarks
still need a different approach. Setup is more complex (building a random
Hamiltonian cycle through cache-line-sized nodes). Also doesn't solve the
L3 residency problem — after a full traversal, lines are still in cache for
the next traversal.

### E. Hardware counters as the source of truth

Instead of trying to guarantee cold caches, instrument the benchmark with
`perf_event_open` counters (cache misses, LLC loads, DRAM accesses) and
validate that the measurement matches what it should. If a sequential read
of 1 GiB should produce ~16M cache-line fills from DRAM, check that it did.
If it didn't, the result is suspect.

**Pro:** doesn't require changing the benchmark mechanics. Works as a
post-hoc validation layer. `verify_memory` already does this.

**Con:** hardware counters are imprecise (sampling, multiplexing), and
"cache miss" definitions vary by microarchitecture. Can validate but not fix
a benchmark that's reading from cache. Also Linux-only for `perf_event_open`.

## Current state

`verify_memory` implements approach E for `seq_read_single` only: flush the
cache, read once, check that miss rate > 90%. This validates a single cold
pass but isn't integrated into the timed benchmark.

The daily benchmarks use none of these approaches. All three memory
measurements likely report cache throughput, not DRAM throughput, on any
machine where the benchmark runs more than one iteration.
