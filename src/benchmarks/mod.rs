pub mod memory;
pub mod hash;
pub mod syscall;
pub mod ssd;
pub mod network;
pub mod sort;
pub mod compression;
pub mod serialization;

use std::time::{Duration, Instant};

pub struct Measurement {
    pub name: &'static str,
    pub latency_ns: Option<f64>,
    pub throughput_bytes_s: Option<f64>,
}

pub fn bench(name: &'static str, size_bytes: usize, duration_secs: u64, mut f: impl FnMut()) -> Measurement {
    let warmup = Duration::from_secs(1);
    let t = Instant::now();
    while t.elapsed() < warmup {
        f();
    }

    let measure = Duration::from_secs(duration_secs);
    let t = Instant::now();
    let mut iters: u64 = 0;
    while t.elapsed() < measure {
        f();
        iters += 1;
    }
    let elapsed = t.elapsed();
    let ns_per_op = elapsed.as_nanos() as f64 / iters as f64;

    Measurement {
        name,
        latency_ns: Some(ns_per_op),
        throughput_bytes_s: if size_bytes > 0 {
            Some(size_bytes as f64 / (ns_per_op / 1e9))
        } else {
            None
        },
    }
}

#[inline(never)]
pub fn black_box<T>(dummy: T) -> T {
    unsafe {
        let ret = std::ptr::read_volatile(&dummy);
        std::mem::forget(dummy);
        ret
    }
}







