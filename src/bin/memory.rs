//! Sequential memory read and write (separate experiments, cold-cache passes).
use napkin_math::benchmarks::memory;

fn main() {
    eprintln!("sequential memory — flush cache before each pass\n");

    for m in [
        memory::seq_read_single(),
        memory::seq_read_threaded(),
        memory::seq_write_single(),
        memory::seq_write_threaded(),
    ] {
        let thr = m
            .throughput_bytes_s
            .map(|b| format!("{:.1} GiB/s", b / 1_073_741_824.0))
            .unwrap_or_else(|| "—".into());
        eprintln!("  {:<30} {}", m.name, thr);
    }
}
