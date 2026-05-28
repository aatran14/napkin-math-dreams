use crate::benchmarks::{black_box, Measurement};
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

type Int = u64;

// Sequential Memory Read (single thread, vectorized)
// Matches sirupsen's master i.e., a criterion bench imploring AVX2 with 4 accumulators on x86_64
// scalar fallback elsewhere. 1 GiB buffer of contiguous u64s
pub fn seq_read_single() -> Measurement {
    let size_bytes: usize = 1024 * 1024 * 1024; // 1 GiB
    let n = size_bytes / 8;
    let vec: Vec<Int> = (0..n).map(|i| i as Int).collect();

    // warmup
    for _ in 0..3 {
        black_box(memory_read_vectorized(&vec));
    }

    let duration = Duration::from_secs(10);
    let t = Instant::now();
    let mut iters: u64 = 0;
    while t.elapsed() < duration {
        black_box(memory_read_vectorized(&vec));
        iters += 1;
    }
    let elapsed = t.elapsed();
    let ns_per_op = elapsed.as_nanos() as f64 / iters as f64;

    Measurement {
        name: "seq_mem_read_single",
        latency_ns: Some(ns_per_op),
        throughput_bytes_s: Some(size_bytes as f64 / (ns_per_op / 1e9)),
    }
}

// Sequential Memory Read (threaded, vectorized)

//  // Pins each thread to a separate core to maximize memory bandwidth
pub fn seq_read_threaded() -> Measurement {
    let size_bytes: usize = 1024 * 1024 * 1024; // 1 GiB
    let n = size_bytes / 8;

    let core_ids = core_affinity::get_core_ids().unwrap_or_default();
    let num_threads = core_ids.len().max(1);
    let slice_size = n / num_threads;

    let start_barrier = Arc::new(Barrier::new(num_threads + 1));
    let end_barrier = Arc::new(Barrier::new(num_threads + 1));
    let done_allocating = Arc::new(Barrier::new(num_threads + 1));
    let done = Arc::new(AtomicBool::new(false));
    let total = Arc::new(AtomicU64::new(0));

    for (idx, core) in core_ids.iter().copied().enumerate() {
        let start_b = start_barrier.clone();
        let end_b = end_barrier.clone();
        let done_alloc = done_allocating.clone();
        let done = done.clone();
        let total = total.clone();

        thread::spawn(move || {
            core_affinity::set_for_current(core);

            let range_start = slice_size * idx;
            let range_end = if idx + 1 == num_threads { n } else { range_start + slice_size };
            let vec: Vec<Int> = (range_start..range_end).map(|i| i as Int).collect();

            done_alloc.wait();
            loop {
                start_b.wait();
                if done.load(Ordering::Relaxed) { return; }
                total.fetch_add(memory_read_vectorized(&vec), Ordering::Relaxed);
                end_b.wait();
            }
        });
    }

    done_allocating.wait();

    // warmup
    for _ in 0..3 {
        total.store(0, Ordering::Relaxed);
        start_barrier.wait();
        end_barrier.wait();
    }

    let duration = Duration::from_secs(10);
    let t = Instant::now();
    let mut iters: u64 = 0;
    while t.elapsed() < duration {
        total.store(0, Ordering::Relaxed);
        start_barrier.wait();
        end_barrier.wait();
        iters += 1;
    }
    let elapsed = t.elapsed();

    done.store(true, Ordering::Relaxed);
    start_barrier.wait();

    let ns_per_op = elapsed.as_nanos() as f64 / iters as f64;

    Measurement {
        name: "seq_mem_read_threaded",
        latency_ns: Some(ns_per_op),
        throughput_bytes_s: Some(size_bytes as f64 / (ns_per_op / 1e9)),
    }
}

// Random Memory Read.
// Shuffled access pattern defeats the prefetcher, so every read is a cache miss to DRAM
pub fn random_read() -> Measurement {
    let bytes_per_iter: usize = 64;
    let n = 1024 * 1024 * 1024usize / bytes_per_iter;
    let vec: Vec<[u64; 8]> = (0..n).map(|i| [i as u64; 8]).collect();

    let mut order: Vec<usize> = (0..n).collect();
    order.shuffle(&mut thread_rng());
    let mut i = 0usize;

    crate::benchmarks::bench("mem_random_rw", bytes_per_iter, 5, || {
        black_box(vec[order[i]]);
        i += 1;
        if i >= order.len() { i = 0; }
    })
}

pub fn run() -> Vec<Measurement> {
    vec![
        seq_read_single(),
        seq_read_threaded(),
        random_read(),
    ]
}

// --- Vectorized memory read implementations ---

#[inline(never)]
fn memory_read_vectorized(vec: &[Int]) -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            unsafe { return memory_read_avx2(vec); }
        }
    }

    // Scalar fallback for ARM and other architectures
    let mut sum = 0u64;
    for value in vec {
        sum = sum.wrapping_add(*value);
    }
    sum
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
