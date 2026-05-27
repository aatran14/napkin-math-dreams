use crate::benchmarks::{bench, black_box, Measurement};

// System call (getpid).
// Cheapest possible syscall, so this is the floor cost of any kernel interaction.
// Spectre/Meltdown mitigations made this 2-5x more expensive on affected CPUs.
pub fn getpid() -> Measurement {
    bench("syscall", 0, 5, || {
        black_box(unsafe { libc::getpid() });
    })
}

// Context switch.
// Unix socket ping-pong forces a context switch on each send/receive.
// Two switches per round trip, so we divide by 2.
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
