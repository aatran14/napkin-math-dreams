//! Memory section only — shorthand for `daily run --section memory`.
use napkin_math::benchmarks::manifest;

fn main() {
    let rows = manifest::list_rows(false, Some("memory"));
    eprintln!("memory benchmarks ({} rows)\n", rows.len());
    for m in manifest::run_rows(&rows) {
        let thr = m
            .throughput_bytes_s
            .map(|b| format!("{:.1} GiB/s", b / 1_073_741_824.0))
            .unwrap_or_else(|| "—".into());
        eprintln!("  {:<30} {}", m.name, thr);
    }
}
