use crate::benchmarks::{black_box, Measurement};
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

type Int = u64;

const BUFFER_BYTES: usize = 1024 * 1024 * 1024;

/* 
Sequential read/write: warm streaming bandwidth (sirupsen/napkin-math style).
Each core reads/writes its own slice with affinity pinned; timed passes do not
flush cache, so throughput reflects what the machine can sustain from memory.
*/

/* Single-thread: one core scans a 1 GiB buffer in a tight loop for 10s.
Reports sustained read bandwidth (GiB/s), not per-cache-line latency.
latency_ns is wall time for one full pass; throughput = BUFFER_BYTES / that time.
*/

pub fn seq_read_single() -> Measurement {
    let n = BUFFER_BYTES / 8;
    let vec: Vec<Int> = (0..n).map(|i| i as Int).collect();
    crate::benchmarks::bench("seq_mem_read_single", BUFFER_BYTES, 10, || {
        black_box(memory_read_vectorized(&vec));
    })
}

/* 
Multi-thread: one pinned thread per physical core (lowest sibling in each HT set),
each scanning its slice of the same 1 GiB.
Barriers keep passes in lockstep; wall time is one full parallel sweep. Throughput is
aggregate bandwidth (all cores combined), same units as single-thread.
*/
fn physical_core_ids() -> Vec<core_affinity::CoreId> {
    let available: std::collections::HashSet<usize> = core_affinity::get_core_ids()
        .unwrap_or_default()
        .into_iter()
        .map(|c| c.id)
        .collect();

    #[cfg(target_os = "linux")]
    let mut physical: Vec<core_affinity::CoreId> = physical_core_ids_linux()
        .unwrap_or_default()
        .into_iter()
        .filter(|c| available.contains(&c.id))
        .collect();

    #[cfg(not(target_os = "linux"))]
    let mut physical: Vec<core_affinity::CoreId> = Vec::new();

    if physical.is_empty() {
        physical = available.into_iter().map(|id| core_affinity::CoreId { id }).collect();
    }
    physical.sort_by_key(|c| c.id);
    physical
}

#[cfg(target_os = "linux")]
fn physical_core_ids_linux() -> Option<Vec<core_affinity::CoreId>> {
    use std::collections::BTreeSet;
    use std::fs;

    let cpu_dir = fs::read_dir("/sys/devices/system/cpu").ok()?;
    let mut sibling_sets: BTreeSet<Vec<u32>> = BTreeSet::new();

    for entry in cpu_dir.flatten() {
        let siblings_path = entry.path().join("topology/thread_siblings_list");
        let contents = fs::read_to_string(&siblings_path).ok()?;
        let mut cpus = parse_thread_siblings(&contents);
        if cpus.is_empty() {
            continue;
        }
        cpus.sort_unstable();
        cpus.dedup();
        sibling_sets.insert(cpus);
    }

    if sibling_sets.is_empty() {
        return None;
    }

    Some(
        sibling_sets
            .into_iter()
            .filter_map(|sibs| sibs.first().copied())
            .map(|id| core_affinity::CoreId { id: id as usize })
            .collect(),
    )
}

#[cfg(target_os = "linux")]
fn parse_thread_siblings(s: &str) -> Vec<u32> {
    let mut cpus = Vec::new();
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((lo, hi)) = part.split_once('-') {
            let lo: u32 = lo.trim().parse().unwrap_or(0);
            let hi: u32 = hi.trim().parse().unwrap_or(lo);
            for id in lo..=hi {
                cpus.push(id);
            }
        } else if let Ok(id) = part.parse() {
            cpus.push(id);
        }
    }
    cpus
}

pub fn seq_read_threaded() -> Measurement {
    let n = BUFFER_BYTES / 8;
    let core_ids = physical_core_ids();
    let num_threads = core_ids.len().max(1);
    let slice_size = n / num_threads;

    let start_barrier = Arc::new(Barrier::new(num_threads + 1));
    let end_barrier = Arc::new(Barrier::new(num_threads + 1));
    let done_allocating = Arc::new(Barrier::new(num_threads + 1));
    let done = Arc::new(AtomicBool::new(false));

    for (idx, core) in core_ids.iter().copied().enumerate() {
        let start_b = start_barrier.clone();
        let end_b = end_barrier.clone();
        let done_alloc = done_allocating.clone();
        let done = done.clone();

        let range_start = slice_size * idx;
        let range_end_exclusive = if idx + 1 == num_threads {
            n
        } else {
            range_start + slice_size
        };
        let vec: Vec<Int> = (range_start..range_end_exclusive).map(|i| i as Int).collect();

        thread::spawn(move || {
            core_affinity::set_for_current(core);

            done_alloc.wait();
            loop {
                start_b.wait();
                if done.load(Ordering::Relaxed) {
                    return;
                }
                black_box(memory_read_vectorized(&vec));
                end_b.wait();
            }
        });
    }

    done_allocating.wait();

    let warmup = Duration::from_secs(1);
    let t = Instant::now();
    while t.elapsed() < warmup {
        start_barrier.wait();
        end_barrier.wait();
    }

    let duration = Duration::from_secs(10);
    let t = Instant::now();
    let mut iters: u64 = 0;
    while t.elapsed() < duration {
        start_barrier.wait();
        end_barrier.wait();
        iters += 1;
    }
    let elapsed = t.elapsed();
    done.store(true, Ordering::Relaxed);
    start_barrier.wait();

    let ns_per_pass = elapsed.as_nanos() as f64 / iters as f64;
    let throughput = BUFFER_BYTES as f64 / (ns_per_pass / 1e9);

    Measurement {
        name: "seq_mem_read_threaded",
        latency_ns: Some(ns_per_pass),
        throughput_bytes_s: Some(throughput),
    }
}

// single threaded susatined write bandwidth.
// this is one core repeatedlly writing 1GiB buffer for 10 seconds. in the run, this  occurs after a 1second warmup.
// each iteration scans the whole buffer with vectorized stores 
// black_box(vec[0]) is a dummy read to stop the compiler form dropping the work. (maybe this could be better, but i'm too dumb to make it complicated).
// the result is Throughput = 1GiB divided by the average pass time.
pub fn seq_write_single() -> Measurement {
    let n = BUFFER_BYTES / 8;
    let mut vec: Vec<Int> = (0..n).map(|i| i as Int).collect();
    crate::benchmarks::bench("seq_mem_write_single", BUFFER_BYTES, 10, || {
        memory_write_vectorized(&mut vec);
        black_box(vec[0]);
    })
}

pub fn seq_write_threaded() -> Measurement {
    let n = BUFFER_BYTES / 8;
    let core_ids = physical_core_ids();
    let num_threads = core_ids.len().max(1);
    let slice_size = n / num_threads;

    let start_barrier = Arc::new(Barrier::new(num_threads + 1));
    let end_barrier = Arc::new(Barrier::new(num_threads + 1));
    let done_allocating = Arc::new(Barrier::new(num_threads + 1));
    let done = Arc::new(AtomicBool::new(false));

    for (idx, core) in core_ids.iter().copied().enumerate() {
        let start_b = start_barrier.clone();
        let end_b = end_barrier.clone();
        let done_alloc = done_allocating.clone();
        let done = done.clone();

        let range_start = slice_size * idx;
        let range_end_exclusive = if idx + 1 == num_threads {
            n
        } else {
            range_start + slice_size
        };
        let mut vec: Vec<Int> = (range_start..range_end_exclusive).map(|i| i as Int).collect();

        thread::spawn(move || {
            core_affinity::set_for_current(core);

            done_alloc.wait();
            loop {
                start_b.wait();
                if done.load(Ordering::Relaxed) {
                    return;
                }
                memory_write_vectorized(&mut vec);
                black_box(vec[0]);
                end_b.wait();
            }
        });
    }

    done_allocating.wait();

    let warmup = Duration::from_secs(1);
    let t = Instant::now();
    while t.elapsed() < warmup {
        start_barrier.wait();
        end_barrier.wait();
    }

    let duration = Duration::from_secs(10);
    let t = Instant::now();
    let mut iters: u64 = 0;
    while t.elapsed() < duration {
        start_barrier.wait();
        end_barrier.wait();
        iters += 1;
    }
    let elapsed = t.elapsed();
    done.store(true, Ordering::Relaxed);
    start_barrier.wait();

    let ns_per_pass = elapsed.as_nanos() as f64 / iters as f64;
    let throughput = BUFFER_BYTES as f64 / (ns_per_pass / 1e9);

    Measurement {
        name: "seq_mem_write_threaded",
        latency_ns: Some(ns_per_pass),
        throughput_bytes_s: Some(throughput),
    }
}

/*
Random read/write: per 64-byte cache-line latency over a 1 GiB buffer.
Access order is shuffled once; each timed op touches one line (read or store).
Reports ns/access (latency_ns) and derived MiB/s from 64 / latency.
*/

struct RandomWalk {
    vec: Vec<[u64; 8]>,
    order: Vec<usize>,
    i: usize,
}

fn random_walk_setup() -> RandomWalk {
    let n = BUFFER_BYTES / 64;
    let mut vec: Vec<[u64; 8]> = vec![[1, 2, 3, 4, 5, 6, 7, 8]; n];
    unsafe {
        libc::madvise(
            vec.as_mut_ptr() as *mut libc::c_void,
            n * std::mem::size_of::<[u64; 8]>(),
            libc::MADV_RANDOM,
        );
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.shuffle(&mut thread_rng());
    RandomWalk { vec, order, i: 0 }
}

fn random_walk_next(w: &mut RandomWalk) -> usize {
    let idx = w.order[w.i];
    w.i += 1;
    if w.i >= w.order.len() {
        w.i = 0;
    }
    idx
}

pub fn random_read() -> Measurement {
    let bytes_per_iter = 64;
    let mut walk = random_walk_setup();
    crate::benchmarks::bench("mem_random_read", bytes_per_iter, 5, || {
        let idx = random_walk_next(&mut walk);
        black_box(walk.vec[idx]);
    })
}

pub fn random_write() -> Measurement {
    let bytes_per_iter = 64;
    let mut walk = random_walk_setup();
    const WRITE_PATTERN: [u64; 8] = [8, 7, 6, 5, 4, 3, 2, 1];
    crate::benchmarks::bench("mem_random_write", bytes_per_iter, 5, || {
        let idx = random_walk_next(&mut walk);
        walk.vec[idx] = WRITE_PATTERN;
        black_box(walk.vec[idx]);
    })
}

pub fn run() -> Vec<Measurement> {
    vec![
        seq_read_single(),
        seq_read_threaded(),
        seq_write_single(),
        seq_write_threaded(),
        random_read(),
        random_write(),
    ]
}

#[inline(never)]
fn memory_read_vectorized(vec: &[Int]) -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            unsafe { return memory_read_avx2(vec); }
        }
    }

    let mut sum = 0u64;
    for value in vec {
        sum = sum.wrapping_add(*value);
    }
    sum
}

#[inline(never)]
fn memory_write_vectorized(vec: &mut [Int]) {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            unsafe {
                memory_write_avx2(vec);
                return;
            }
        }
    }

    for value in vec.iter_mut() {
        *value = value.wrapping_add(1);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn memory_read_avx2(vec: &[Int]) -> u64 {
    use std::arch::x86_64::*;

    let mut acc0 = _mm256_setzero_si256();
    let mut acc1 = _mm256_setzero_si256();
    let mut acc2 = _mm256_setzero_si256();
    let mut acc3 = _mm256_setzero_si256();
    let mut i = 0usize;
    let ptr = vec.as_ptr();

    while i + 16 <= vec.len() {
        acc0 = _mm256_add_epi64(acc0, _mm256_loadu_si256(ptr.add(i) as *const __m256i));
        acc1 = _mm256_add_epi64(acc1, _mm256_loadu_si256(ptr.add(i + 4) as *const __m256i));
        acc2 = _mm256_add_epi64(acc2, _mm256_loadu_si256(ptr.add(i + 8) as *const __m256i));
        acc3 = _mm256_add_epi64(acc3, _mm256_loadu_si256(ptr.add(i + 12) as *const __m256i));
        i += 16;
    }

    acc0 = _mm256_add_epi64(acc0, acc1);
    acc2 = _mm256_add_epi64(acc2, acc3);
    acc0 = _mm256_add_epi64(acc0, acc2);

    let mut lanes = [0u64; 4];
    _mm256_storeu_si256(lanes.as_mut_ptr() as *mut __m256i, acc0);

    let mut sum = lanes.iter().copied().sum::<u64>();
    while i < vec.len() {
        sum = sum.wrapping_add(*ptr.add(i));
        i += 1;
    }

    sum
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
unsafe fn memory_write_avx2(vec: &mut [Int]) {
    use std::arch::x86_64::*;

    let pattern = _mm256_set1_epi64x(0x5a5a5a5a5a5a5a5a_i64);
    let mut i = 0usize;
    let ptr = vec.as_mut_ptr();

    while i + 16 <= vec.len() {
        _mm256_storeu_si256(ptr.add(i) as *mut __m256i, pattern);
        _mm256_storeu_si256(ptr.add(i + 4) as *mut __m256i, pattern);
        _mm256_storeu_si256(ptr.add(i + 8) as *mut __m256i, pattern);
        _mm256_storeu_si256(ptr.add(i + 12) as *mut __m256i, pattern);
        i += 16;
    }

    while i < vec.len() {
        *ptr.add(i) = 0x5a5a5a5a5a5a5a5a;
        i += 1;
    }
}
