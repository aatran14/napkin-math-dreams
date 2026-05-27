use crate::benchmarks::{bench, black_box, Measurement};
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::sync::{Arc, Barrier};
use std::thread;

// Sequential Memory Read (single thread ver.)
// 4 GiB buffer exceeds L3 cache (typically 30-50 MiB), forcing reads to DRAM
pub fn seq_read_single() -> Measurement {
    let bytes_per_iter: usize = 64;
    let n = 4 * 1024 * 1024 * 1024usize / bytes_per_iter;
    let vec: Vec<[u64; 8]> = (0..n).map(|i| [i as u64; 8]).collect();
    let mut i = 0usize;

    bench("seq_mem_read_single", bytes_per_iter, 5, || {
        black_box(vec[i]);
        i += 1;
        if i >= vec.len() { i = 0; }
    })
}

// Sequential Memory Read (threaded ver.)
// All cores reading in parallel to saturate the memory bus
pub fn seq_read_threaded() -> Measurement {
    let bytes_per_iter: usize = 64;
    let n = 4 * 1024 * 1024 * 1024usize / bytes_per_iter;
    let num_threads = num_cpus();

    let vec: Arc<Vec<[u64; 8]>> = Arc::new((0..n).map(|i| [i as u64; 8]).collect());
    let total_bytes = n * bytes_per_iter;

    let start_barrier = Arc::new(Barrier::new(num_threads + 1));
    let end_barrier = Arc::new(Barrier::new(num_threads + 1));
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let slice_size = n / num_threads;
    for t in 0..num_threads {
        let vec = vec.clone();
        let start_b = start_barrier.clone();
        let end_b = end_barrier.clone();
        let done = done.clone();
        let lo = t * slice_size;
        let hi = if t == num_threads - 1 { n } else { lo + slice_size };

        thread::spawn(move || {
            loop {
                start_b.wait();
                if done.load(std::sync::atomic::Ordering::Relaxed) { return; }
                let mut sum = 0u64;
                for i in lo..hi {
                    sum = sum.wrapping_add(vec[i][0]);
                }
                black_box(sum);
                end_b.wait();
            }
        });
    }

    // warmup
    for _ in 0..3 {
        start_barrier.wait();
        end_barrier.wait();
    }

    // measure
    let duration = std::time::Duration::from_secs(5);
    let t = std::time::Instant::now();
    let mut iters: u64 = 0;
    while t.elapsed() < duration {
        start_barrier.wait();
        end_barrier.wait();
        iters += 1;
    }
    let elapsed = t.elapsed();

    done.store(true, std::sync::atomic::Ordering::Relaxed);
    start_barrier.wait(); // release threads to exit

    let ns_per_op = elapsed.as_nanos() as f64 / iters as f64;

    Measurement {
        name: "seq_mem_read_threaded",
        latency_ns: Some(ns_per_op),
        throughput_bytes_s: Some(total_bytes as f64 / (ns_per_op / 1e9)),
    }
}

// Random Memory Read.
// Shuffled access pattern defeats the prefetcher, so every read is a cache miss to DRAM.
// This measures memory latency, not bandwidth.
pub fn random_read() -> Measurement {
    let bytes_per_iter: usize = 64;
    let n = 1024 * 1024 * 1024usize / bytes_per_iter;
    let vec: Vec<[u64; 8]> = (0..n).map(|i| [i as u64; 8]).collect();

    let mut order: Vec<usize> = (0..n).collect();
    order.shuffle(&mut thread_rng());
    let mut i = 0usize;

    bench("mem_random_rw", bytes_per_iter, 5, || {
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

fn num_cpus() -> usize {
    thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}
