//! One front door for benchmarks.
//!
//!   daily                          run all nightly rows, append CSV
//!   daily list                     list manifest rows
//!   daily list --nightly           list nightly rows only
//!   daily run memory/random_read   run one row (dev loop)
//!   daily run --section memory     run every row in a section
use napkin_math::benchmarks::{self, manifest};
use std::env;
use std::fs;
use std::io::Write;
use std::process::Command;
use std::time::SystemTime;

fn main() {
    let args: Vec<String> = env::args().collect();
    match parse_command(&args[1..]) {
        CliCommand::List { nightly_only, section } => cmd_list(nightly_only, section.as_deref()),
        CliCommand::Run { ids, section, write_csv } => cmd_run(ids, section.as_deref(), write_csv),
        CliCommand::Nightly => cmd_run(vec![], None, true),
    }
}

enum CliCommand {
    List {
        nightly_only: bool,
        section: Option<String>,
    },
    Run {
        ids: Vec<String>,
        section: Option<String>,
        write_csv: bool,
    },
    Nightly,
}

fn parse_command(args: &[String]) -> CliCommand {
    if args.is_empty() {
        return CliCommand::Nightly;
    }

    match args[0].as_str() {
        "list" => {
            let mut nightly_only = false;
            let mut section = None;
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--nightly" => nightly_only = true,
                    "--section" => {
                        i += 1;
                        section = Some(
                            args.get(i)
                                .unwrap_or_else(|| {
                                    eprintln!("daily list: --section requires a value");
                                    std::process::exit(1);
                                })
                                .clone(),
                        );
                    }
                    other => {
                        eprintln!("daily list: unknown flag {}", other);
                        std::process::exit(1);
                    }
                }
                i += 1;
            }
            CliCommand::List {
                nightly_only,
                section,
            }
        }
        "run" => {
            let mut write_csv = false;
            let mut section = None;
            let mut ids = Vec::new();
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--csv" => write_csv = true,
                    "--section" => {
                        i += 1;
                        section = Some(
                            args.get(i)
                                .unwrap_or_else(|| {
                                    eprintln!("daily run: --section requires a value");
                                    std::process::exit(1);
                                })
                                .clone(),
                        );
                    }
                    id => ids.push(id.to_string()),
                }
                i += 1;
            }
            CliCommand::Run {
                ids,
                section,
                write_csv,
            }
        }
        other => {
            eprintln!("daily: unknown subcommand {:?}", other);
            print_usage();
            std::process::exit(1);
        }
    }
}

fn print_usage() {
    eprintln!(
        "usage:\n\
         \n\
         \tdaily\n\
         \tdaily list [--nightly] [--section NAME]\n\
         \tdaily run [--csv] [--section NAME] [ID...]\n\
         \n\
         examples:\n\
         \tdaily run memory/random_read\n\
         \tdaily run --section memory\n\
         \tdaily list --nightly"
    );
}

fn cmd_list(nightly_only: bool, section: Option<&str>) {
    eprintln!("benchmark manifest ({})", manifest::benchmarks_root().display());
    eprintln!();
    for row in manifest::list_rows(nightly_only, section) {
        eprintln!(
            "  {:<32} nightly={}  {}",
            row.id,
            row.nightly,
            row.path.display()
        );
    }
}

fn cmd_run(ids: Vec<String>, section: Option<&str>, write_csv: bool) {
    let rows = if !ids.is_empty() {
        let resolved = manifest::resolve_ids(&ids);
        manifest::load_rows()
            .into_iter()
            .filter(|r| resolved.iter().any(|id| id == &r.id))
            .collect::<Vec<_>>()
    } else if let Some(section) = section {
        manifest::list_rows(false, Some(section))
    } else if write_csv {
        manifest::list_rows(true, None)
    } else {
        eprintln!("daily run: pass row ids or --section");
        print_usage();
        std::process::exit(1);
    };

    if rows.is_empty() {
        eprintln!("daily run: no rows matched");
        std::process::exit(1);
    }

    if write_csv {
        print_run_header();
    } else {
        eprintln!("napkin-math run");
        for row in &rows {
            eprintln!("  {}", row.id);
        }
        eprintln!();
    }

    let results = manifest::run_rows(&rows);
    print_results(&results);

    if write_csv {
        write_csv_rows(&results);
    }
}

fn print_run_header() {
    let machine = env::var("NAPKIN_MACHINE").unwrap_or_else(|_| hostname());
    let config = env::var("NAPKIN_CONFIG").unwrap_or_else(|_| "baseline".into());
    let csv_path = env::var("NAPKIN_CSV").unwrap_or_else(|_| "data/dead.csv".into());
    let commit = env::var("NAPKIN_COMMIT").unwrap_or_default();
    let date = env::var("NAPKIN_TIMESTAMP").unwrap_or_else(|_| today());

    eprintln!("napkin-math daily run");
    eprintln!("  date:    {}", date);
    eprintln!("  machine: {}", machine);
    eprintln!("  config:  {}", config);
    eprintln!("  csv:     {}", csv_path);
    if !commit.is_empty() {
        eprintln!("  commit:  {}", commit);
    }
    if let Some(cpu) = cpu_model() {
        eprintln!("  cpu:     {}", cpu);
    }
    eprintln!();
}

fn print_results(results: &[benchmarks::Measurement]) {
    eprintln!("{} measurements", results.len());
    eprintln!();
    for m in results {
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
    eprintln!();
}

fn write_csv_rows(results: &[benchmarks::Measurement]) {
    let machine = env::var("NAPKIN_MACHINE").unwrap_or_else(|_| hostname());
    let config = env::var("NAPKIN_CONFIG").unwrap_or_else(|_| "baseline".into());
    let cpu = cpu_model().unwrap_or_default();
    let commit = env::var("NAPKIN_COMMIT").unwrap_or_default();
    let date = env::var("NAPKIN_TIMESTAMP").unwrap_or_else(|_| today());
    let csv_path = env::var("NAPKIN_CSV").unwrap_or_else(|_| "data/dead.csv".into());

    let needs_header = !std::path::Path::new(&csv_path).exists()
        || fs::metadata(&csv_path)
            .map(|m| m.len() == 0)
            .unwrap_or(true);

    if let Some(parent) = std::path::Path::new(&csv_path).parent() {
        let _ = fs::create_dir_all(parent);
    }

    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&csv_path)
        .expect("open csv");

    if needs_header {
        writeln!(
            f,
            "date,machine,cpu,config,operation,latency_ns,throughput_bytes_s,commit"
        )
        .unwrap();
    }

    for m in results {
        let lat = m.latency_ns.map(|v| format!("{:.2}", v)).unwrap_or_default();
        let thr = m
            .throughput_bytes_s
            .map(|v| format!("{:.0}", v))
            .unwrap_or_default();
        writeln!(
            f,
            "{},{},{},{},{},{},{},{}",
            date, machine, &cpu, config, m.name, lat, thr, commit
        )
        .unwrap();
    }

    eprintln!("wrote {} rows to {}", results.len(), csv_path);
}

fn today() -> String {
    let output = Command::new("date").arg("+%Y-%m-%dT%H:%M:%S").output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => {
            let now = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            format!("{}", 1970 + (now / 86400) * 400 / 146097)
        }
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
                return rest
                    .split_once(':')
                    .map(|(_, v)| clean_cpu_name(v.trim()));
            }
        }
    }
    if let Ok(output) = Command::new("lscpu").output() {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                if let Some(rest) = line.strip_prefix("Model name:") {
                    return Some(clean_cpu_name(rest.trim()));
                }
            }
        }
    }
    Command::new("sysctl")
        .args(["-n", "machdep.cpu.brand_string"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| clean_cpu_name(String::from_utf8_lossy(&o.stdout).trim()))
}

fn clean_cpu_name(raw: &str) -> String {
    let s = raw
        .replace("(R)", "")
        .replace("(TM)", "")
        .replace("(tm)", "")
        .replace("CPU ", "")
        .replace("Processor", "");
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if let Some(i) = s.find(" @") {
        s[..i].trim().to_string()
    } else {
        s.trim().to_string()
    }
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
