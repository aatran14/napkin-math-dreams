//! README row: Sequential Memory R/W — read only (write not implemented).
use napkin_math::benchmarks::memory;

fn main() {
    for m in [memory::seq_read_single(), memory::seq_read_threaded()] {
        let thr = m
            .throughput_bytes_s
            .map(|b| format!("{:.1} GiB/s", b / 1_073_741_824.0))
            .unwrap_or_else(|| "—".into());
        eprintln!("  {:<30} {}", m.name, thr);
    }
    eprintln!("\nfull table: cargo run --release --bin readme");
}
