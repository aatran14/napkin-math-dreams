use napkin_math::benchmarks::{self, Measurement};
use std::env;
use std::fs;
use std::io::Write;
use std::process::Command;
use std::time::SystemTime;

fn main() {
    let machine = env::var("NAPKIN_MACHINE").unwrap_or_else(|_| hostname());
    let config = env::var("NAPKIN_CONFIG").unwrap_or_else(|_| "baseline".into());
    let date = today();
    let csv_path = env::var("NAPKIN_CSV").unwrap_or_else(|_| "data/dead.csv".into());

    eprintln!("napkin-math daily run");
    eprintln!("  date:    {}", date);
    eprintln!("  machine: {}", machine);
    eprintln!("  config:  {}", config);
    eprintln!("  csv:     {}", csv_path);
    if let Some(cpu) = cpu_model() {
        eprintln!("  cpu:     {}", cpu);
    }
    eprintln!();

    let mut results: Vec<Measurement> = Vec::new();

    eprintln!("running memory benchmarks...");
    results.extend(benchmarks::memory::run());

    eprintln!("running hash benchmarks...");
    results.extend(benchmarks::hash::run());

    eprintln!("running syscall benchmarks...");
    results.extend(benchmarks::syscall::run());

    eprintln!("running ssd benchmarks...");
    results.extend(benchmarks::ssd::run());

    eprintln!("running network benchmarks...");
    results.extend(benchmarks::network::run());

    eprintln!("running sort benchmarks...");
    results.extend(benchmarks::sort::run());

    eprintln!("running compression benchmarks...");
    results.extend(benchmarks::compression::run());

    eprintln!("running serialization benchmarks...");
    results.extend(benchmarks::serialization::run());

    eprintln!();
    eprintln!("{} measurements collected", results.len());

    // print summary to stderr
    for m in &results {
        let lat = m.latency_ns.map(|v| format_latency(v)).unwrap_or_else(|| "—".into());
        let thr = m.throughput_bytes_s.map(|v| format_throughput(v)).unwrap_or_else(|| "—".into());
        eprintln!("  {:<30} lat: {:>12}  thr: {:>12}", m.name, lat, thr);
    }

    // write CSV
    let needs_header = !std::path::Path::new(&csv_path).exists()
        || fs::metadata(&csv_path).map(|m| m.len() == 0).unwrap_or(true);

    if let Some(parent) = std::path::Path::new(&csv_path).parent() {
        let _ = fs::create_dir_all(parent);
    }

    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&csv_path)
        .expect("open csv");

    if needs_header {
        writeln!(f, "date,machine,config,operation,latency_ns,throughput_bytes_s").unwrap();
    }

    for m in &results {
        let lat = m.latency_ns.map(|v| format!("{:.2}", v)).unwrap_or_default();
        let thr = m.throughput_bytes_s.map(|v| format!("{:.0}", v)).unwrap_or_default();
        writeln!(f, "{},{},{},{},{},{}", date, machine, config, m.name, lat, thr).unwrap();
    }

    eprintln!("wrote {} rows to {}", results.len(), csv_path);
}

fn today() -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let days = now / 86400;
    let y = 1970 + (days * 400 / 146097); // approximate, good enough
    // use chrono-free date formatting
    let output = Command::new("date").arg("+%Y-%m-%d").output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => format!("{}", y),
    }
}

fn hostname() -> String {
    Command::new("hostname")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into())
}

fn cpu_model() -> Option<String> {
    if let Ok(cpuinfo) = fs::read_to_string("/proc/cpuinfo") {
        for line in cpuinfo.lines() {
            if let Some(rest) = line.strip_prefix("model name") {
                return rest.split_once(':').map(|(_, v)| v.trim().to_string());
            }
        }
    }
    Command::new("sysctl")
        .args(["-n", "machdep.cpu.brand_string"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn format_latency(ns: f64) -> String {
    if ns < 1_000.0 { format!("{:.1} ns", ns) }
    else if ns < 1_000_000.0 { format!("{:.1} us", ns / 1_000.0) }
    else if ns < 1_000_000_000.0 { format!("{:.1} ms", ns / 1_000_000.0) }
    else { format!("{:.2} s", ns / 1_000_000_000.0) }
}

fn format_throughput(bytes_s: f64) -> String {
    if bytes_s >= 1_073_741_824.0 { format!("{:.1} GiB/s", bytes_s / 1_073_741_824.0) }
    else if bytes_s >= 1_048_576.0 { format!("{:.1} MiB/s", bytes_s / 1_048_576.0) }
    else { format!("{:.1} KiB/s", bytes_s / 1_024.0) }
}
