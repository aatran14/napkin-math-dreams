use napkin_math::benchmarks::{manifest, Measurement};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let section = parse_section(&args[1..]);

    eprintln!("napkin-math benchmarks\n");
    for sec in manifest::sections(true) {
        if let Some(name) = section {
            if sec.name != name {
                continue;
            }
        }
        eprintln!("[{}]", sec.name);
        for m in manifest::run_rows(&sec.rows) {
            print_row(&m);
        }
        eprintln!();
    }
}

fn parse_section(args: &[String]) -> Option<&str> {
    if args.is_empty() {
        return None;
    }
    if args[0] == "--section" {
        return args.get(1).map(|s| s.as_str());
    }
    Some(args[0].as_str())
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
