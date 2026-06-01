use napkin_math::benchmarks::{self, Measurement};

fn main() {
    eprintln!("napkin-math benchmarks\n");
    for section in benchmarks::readme::sections() {
        eprintln!("[{}]", section.name);
        for m in &section.measurements {
            print_row(m);
        }
        eprintln!();
    }
}

fn print_row(m: &Measurement) {
    let lat = m
        .latency_ns
        .map(format_latency)
        .unwrap_or_else(|| "—".into());
    let thr = m
        .throughput_bytes_s
        .map(format_throughput)
        .unwrap_or_else(|| "—".into());
    eprintln!("  {:<30} lat: {:>12}  thr: {:>12}", m.name, lat, thr);
}

fn format_latency(ns: f64) -> String {
    if ns < 1_000.0 {
        format!("{:.1} ns", ns)
    } else if ns < 1_000_000.0 {
        format!("{:.1} us", ns / 1_000.0)
    } else if ns < 1_000_000_000.0 {
        format!("{:.1} ms", ns / 1_000_000.0)
    } else {
        format!("{:.2} s", ns / 1_000_000_000.0)
    }
}

fn format_throughput(bytes_s: f64) -> String {
    if bytes_s >= 1_073_741_824.0 {
        format!("{:.1} GiB/s", bytes_s / 1_073_741_824.0)
    } else if bytes_s >= 1_048_576.0 {
        format!("{:.1} MiB/s", bytes_s / 1_048_576.0)
    } else {
        format!("{:.1} KiB/s", bytes_s / 1_024.0)
    }
}
