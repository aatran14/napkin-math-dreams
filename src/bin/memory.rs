use napkin_math::benchmarks::{self, Measurement};

fn main() {
    eprintln!("sequential memory read (the table row you're on)\n");

    print_result("single thread", benchmarks::memory::seq_read_single());
    print_result("threaded", benchmarks::memory::seq_read_threaded());

    eprintln!("\nwrite: not implemented yet");
}

fn print_result(label: &str, m: Measurement) {
    let thr = m
        .throughput_bytes_s
        .map(|b| format!("{:.1} GiB/s", b / 1_073_741_824.0))
        .unwrap_or_else(|| "—".into());
    eprintln!("  {} — {}", label, thr);
}
