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
We aim to measure DRAM bandwidth. To do this we must 
- measure the time it takes to read
- measure the time it takes to write

the standard convention is that these operations are the same.
Even sirupsen/napkin-math measures only the read path, but for 
but for even more transparency, memory.rs perform both R/W throughput

R/W is difficult to isolate on memory because of the cache layer.
This demands a method to either exhaust the cache 


*/
// Sequential read/write: separate experiments. Each timed sample flushes cache,
// then measures only the pass time (read or write), so throughput ≈ DRAM ceiling.

pub fn seq_read_single() -> Measurement {
    let n = BUFFER_BYTES / 8;
    let vec: Vec<Int> = (0..n).map(|i| i as Int).collect();
    let (ns_per_pass, throughput) = measure_cold_passes(BUFFER_BYTES, 10, || {
        unsafe { flush_cache(&vec) };
        let t = Instant::now();
        black_box(memory_read_vectorized(&vec));
        t.elapsed()
    });
    Measurement {
        name: "seq_mem_read_single",
        latency_ns: Some(ns_per_pass),
        throughput_bytes_s: Some(throughput),
    }
}

pub fn seq_read_threaded() -> Measurement {
    let n = BUFFER_BYTES / 8;
    let core_ids = core_affinity::get_core_ids().unwrap_or_default();
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

        thread::spawn(move || {
            core_affinity::set_for_current(core);
            let range_start = slice_size * idx;
            let range_end = if idx + 1 == num_threads {
                n
            } else {
                range_start + slice_size
            };
            let vec: Vec<Int> = (range_start..range_end).map(|i| i as Int).collect();

            done_alloc.wait();
            loop {
                start_b.wait();
                if done.load(Ordering::Relaxed) {
                    return;
                }
                unsafe { flush_cache(&vec) };
                black_box(memory_read_vectorized(&vec));
                end_b.wait();
            }
        });
    }

    done_allocating.wait();

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

pub fn seq_write_single() -> Measurement {
    let n = BUFFER_BYTES / 8;
    let mut vec: Vec<Int> = (0..n).map(|i| i as Int).collect();
    let (ns_per_pass, throughput) = measure_cold_passes(BUFFER_BYTES, 10, || {
        unsafe { flush_cache(&vec) };
        let t = Instant::now();
        memory_write_vectorized(&mut vec);
        black_box(vec[0]);
        t.elapsed()
    });
    Measurement {
        name: "seq_mem_write_single",
        latency_ns: Some(ns_per_pass),
        throughput_bytes_s: Some(throughput),
    }
}

pub fn seq_write_threaded() -> Measurement {
    let n = BUFFER_BYTES / 8;
    let core_ids = core_affinity::get_core_ids().unwrap_or_default();
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

        thread::spawn(move || {
            core_affinity::set_for_current(core);
            let range_start = slice_size * idx;
            let range_end = if idx + 1 == num_threads {
                n
            } else {
                range_start + slice_size
            };
            let mut vec: Vec<Int> = (range_start..range_end).map(|i| i as Int).collect();

            done_alloc.wait();
            loop {
                start_b.wait();
                if done.load(Ordering::Relaxed) {
                    return;
                }
                unsafe { flush_cache(&vec) };
                memory_write_vectorized(&mut vec);
                black_box(vec[0]);
                end_b.wait();
            }
        });
    }

    done_allocating.wait();

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

pub fn random_read() -> Measurement {
    let bytes_per_iter: usize = 64;
    let n = BUFFER_BYTES / bytes_per_iter;
    let vec: Vec<[u64; 8]> = (0..n).map(|i| [i as u64; 8]).collect();

    let mut order: Vec<usize> = (0..n).collect();
    order.shuffle(&mut thread_rng());
    let mut i = 0usize;

    crate::benchmarks::bench("mem_random_rw", bytes_per_iter, 5, || {
        black_box(vec[order[i]]);
        i += 1;
        if i >= order.len() {
            i = 0;
        }
    })
}

pub fn run() -> Vec<Measurement> {
    vec![
        seq_read_single(),
        seq_read_threaded(),
        seq_write_single(),
        seq_write_threaded(),
        random_read(),
    ]
}

fn measure_cold_passes<F>(size_bytes: usize, duration_secs: u64, mut pass: F) -> (f64, f64)
where
    F: FnMut() -> Duration,
{
    let deadline = Instant::now() + Duration::from_secs(duration_secs);
    let mut pass_ns: u128 = 0;
    let mut samples: u64 = 0;
    while Instant::now() < deadline {
        pass_ns += pass().as_nanos() as u128;
        samples += 1;
    }
    let ns_per_pass = pass_ns as f64 / samples as f64;
    let throughput = size_bytes as f64 / (ns_per_pass / 1e9);
    (ns_per_pass, throughput)
}

// Evict buffer from cache so the next pass measures DRAM, not L3.
unsafe fn flush_cache(data: &[Int]) {
    let ptr = data.as_ptr() as *const u8;
    let len = data.len() * 8;
    let mut offset = 0usize;

    while offset < len {
        #[cfg(target_arch = "x86_64")]
        std::arch::asm!(
            "clflushopt [{addr}]",
            addr = in(reg) ptr.add(offset),
            options(nostack, preserves_flags),
        );

        #[cfg(target_arch = "aarch64")]
        std::arch::asm!(
            "dc civac, {addr}",
            addr = in(reg) ptr.add(offset),
            options(nostack, preserves_flags),
        );

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        {
            let _ = ptr.add(offset);
        }

        offset += 64;
    }

    #[cfg(target_arch = "x86_64")]
    std::arch::asm!("mfence", options(nostack, preserves_flags));

    #[cfg(target_arch = "aarch64")]
    std::arch::asm!("dsb sy", options(nostack, preserves_flags));
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
