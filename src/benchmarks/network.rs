use crate::benchmarks::{black_box, Measurement};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

// TCP echo server (32 KiB), localhost.
// Measures the kernel's TCP stack overhead without real network latency.
pub fn tcp_echo() -> Measurement {
    let buf_size: usize = 32 * 1024;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();

    thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else { return };
        stream.set_nodelay(true).unwrap();
        let mut buf = vec![0u8; buf_size];
        loop {
            match stream.read(&mut buf) {
                Ok(0) | Err(_) => return,
                Ok(n) => { if stream.write_all(&buf[..n]).is_err() { return; } }
            }
        }
    });

    let mut stream = TcpStream::connect(addr).unwrap();
    stream.set_nodelay(true).unwrap();
    let request = vec![0xA5u8; buf_size];
    let mut response = vec![0u8; buf_size];

    // warmup
    for _ in 0..1000 {
        stream.write_all(&request).unwrap();
        stream.read_exact(&mut response).unwrap();
    }

    let duration = Duration::from_secs(5);
    let t = Instant::now();
    let mut iters: u64 = 0;
    while t.elapsed() < duration {
        stream.write_all(&request).unwrap();
        stream.read_exact(&mut response).unwrap();
        black_box(response[0]);
        iters += 1;
    }
    let elapsed = t.elapsed();
    let ns_per_op = elapsed.as_nanos() as f64 / iters as f64;

    Measurement {
        name: "tcp_echo",
        latency_ns: Some(ns_per_op),
        throughput_bytes_s: Some(buf_size as f64 / (ns_per_op / 1e9)),
    }
}

pub fn run() -> Vec<Measurement> {
    vec![tcp_echo()]
}
