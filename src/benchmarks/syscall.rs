use crate::benchmarks::{bench, black_box, Measurement};

// System Call
// README: ~300 ns latency
pub fn getpid() -> Measurement {
    bench("syscall", 0, 5, || {
        black_box(unsafe { libc::getpid() });
    })
}

// Context Switch
// README: ~10 μs latency
//
// Measures thread-to-thread context switch via a pipe ping-pong.
// Each iteration: write one byte on thread A, read it on thread B,
// write back, read on A. That's two context switches per round trip.
pub fn context_switch() -> Measurement {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::thread;
    use std::time::{Duration, Instant};

    let (mut parent, mut child) = UnixStream::pair().expect("socketpair");
    let buf_write = [0xABu8; 1];

    let handle = thread::spawn(move || {
        let mut buf = [0u8; 1];
        loop {
            if child.read_exact(&mut buf).is_err() { return; }
            if child.write_all(&buf).is_err() { return; }
        }
    });

    // warmup
    let mut buf_read = [0u8; 1];
    for _ in 0..10_000 {
        parent.write_all(&buf_write).unwrap();
        parent.read_exact(&mut buf_read).unwrap();
    }

    let duration = Duration::from_secs(5);
    let t = Instant::now();
    let mut iters: u64 = 0;
    while t.elapsed() < duration {
        parent.write_all(&buf_write).unwrap();
        parent.read_exact(&mut buf_read).unwrap();
        iters += 1;
    }
    let elapsed = t.elapsed();

    drop(parent);
    let _ = handle.join();

    // each iteration has 2 context switches (parent->child, child->parent)
    let ns_per_switch = elapsed.as_nanos() as f64 / (iters as f64 * 2.0);

    Measurement {
        name: "context_switch",
        latency_ns: Some(ns_per_switch),
        throughput_bytes_s: None,
    }
}

pub fn run() -> Vec<Measurement> {
    vec![getpid(), context_switch()]
}
