use crate::benchmarks::{Measurement};
use std::time::{Duration, Instant};

// Sort 1 MiB of random u64s.
// Fits in L2/L3 cache, so this is CPU-bound (branch prediction, comparisons).
// sort_unstable avoids allocation overhead from the stable sort's merge buffer.
pub fn sort_u64() -> Measurement {
    let n = 1024 * 1024 / 8;
    let size_bytes = n * 8;
    let original: Vec<u64> = (0..n).map(|_| rand::random::<u64>()).collect();

    // warmup
    for _ in 0..10 {
        let mut v = original.clone();
        v.sort_unstable();
    }

    let duration = Duration::from_secs(5);
    let t = Instant::now();
    let mut iters: u64 = 0;
    while t.elapsed() < duration {
        let mut v = original.clone();
        v.sort_unstable();
        iters += 1;
    }
    let elapsed = t.elapsed();
    let ns_per_op = elapsed.as_nanos() as f64 / iters as f64;

    Measurement {
        name: "sort_64bit",
        latency_ns: None,
        throughput_bytes_s: Some(size_bytes as f64 / (ns_per_op / 1e9)),
    }
}

pub fn run() -> Vec<Measurement> {
    vec![sort_u64()]
}
