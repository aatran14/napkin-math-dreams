use std::env;
use std::error::Error;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::Instant;

trait Probe {
    fn name(&self) -> &'static str;
    fn default_batch(&self) -> usize;
    fn run_once(&mut self);
}

struct Config {
    probe: String,
    samples: usize,
    warmup_samples: usize,
    batch: Option<usize>,
    buckets: usize,
    csv: Option<String>,
    size_bytes: Option<usize>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            probe: String::from("syscall_getpid"),
            samples: 100_000,
            warmup_samples: 10_000,
            batch: None,
            buckets: 32,
            csv: None,
            size_bytes: None,
        }
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {}", err);
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let Some(config) = parse_args()? else {
        return Ok(());
    };

    let mut probe = make_probe(&config)?;
    let batch = config.batch.unwrap_or_else(|| probe.default_batch());

    println!("Napkin Math latency sampler");
    println!("probe: {}", probe.name());
    println!("samples: {}", config.samples);
    println!("batch: {}", batch);
    println!("warmup samples: {}", config.warmup_samples);
    print_context();

    let mut overhead_probe = NoopProbe;
    let overhead_samples = measure_samples(
        &mut overhead_probe,
        config.warmup_samples,
        config.samples.min(10_000),
        batch,
    );
    let overhead = percentile(&sorted(&overhead_samples), 0.50);

    let raw_samples = measure_samples(probe.as_mut(), config.warmup_samples, config.samples, batch);
    let adjusted_samples: Vec<u64> = raw_samples
        .iter()
        .map(|sample| sample.saturating_sub(overhead))
        .collect();
    let sorted_samples = sorted(&adjusted_samples);

    println!();
    println!("timer plus loop overhead p50: {}", format_latency(overhead));
    print_summary(&sorted_samples);
    println!();
    print_histogram(&sorted_samples, config.buckets);

    if let Some(path) = &config.csv {
        write_csv(path, &raw_samples, &adjusted_samples)?;
        println!();
        println!("wrote raw samples to {}", path);
    }

    Ok(())
}

fn parse_args() -> Result<Option<Config>, Box<dyn Error>> {
    let mut config = Config::default();
    let mut args = env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(None);
            }
            "--list" => {
                print_probe_list();
                return Ok(None);
            }
            "--probe" => {
                config.probe = next_value(&mut args, "--probe")?;
            }
            "--samples" => {
                config.samples = next_value(&mut args, "--samples")?.parse()?;
            }
            "--warmup-samples" => {
                config.warmup_samples = next_value(&mut args, "--warmup-samples")?.parse()?;
            }
            "--batch" => {
                config.batch = Some(next_value(&mut args, "--batch")?.parse()?);
            }
            "--buckets" => {
                config.buckets = next_value(&mut args, "--buckets")?.parse()?;
            }
            "--csv" => {
                config.csv = Some(next_value(&mut args, "--csv")?);
            }
            "--size" => {
                config.size_bytes = Some(parse_size(&next_value(&mut args, "--size")?)?);
            }
            _ if arg.starts_with("--probe=") => {
                config.probe = arg["--probe=".len()..].to_string();
            }
            _ if arg.starts_with("--samples=") => {
                config.samples = arg["--samples=".len()..].parse()?;
            }
            _ if arg.starts_with("--warmup-samples=") => {
                config.warmup_samples = arg["--warmup-samples=".len()..].parse()?;
            }
            _ if arg.starts_with("--batch=") => {
                config.batch = Some(arg["--batch=".len()..].parse()?);
            }
            _ if arg.starts_with("--buckets=") => {
                config.buckets = arg["--buckets=".len()..].parse()?;
            }
            _ if arg.starts_with("--csv=") => {
                config.csv = Some(arg["--csv=".len()..].to_string());
            }
            _ if arg.starts_with("--size=") => {
                config.size_bytes = Some(parse_size(&arg["--size=".len()..])?);
            }
            _ => {
                return Err(format!("unknown argument: {}", arg).into());
            }
        }
    }

    if config.samples == 0 {
        return Err("--samples must be greater than zero".into());
    }
    if config.buckets == 0 {
        return Err("--buckets must be greater than zero".into());
    }
    if matches!(config.batch, Some(0)) {
        return Err("--batch must be greater than zero".into());
    }

    Ok(Some(config))
}

fn next_value<I>(args: &mut I, flag: &str) -> Result<String, Box<dyn Error>>
where
    I: Iterator<Item = String>,
{
    args.next()
        .ok_or_else(|| format!("{} requires a value", flag).into())
}

fn print_help() {
    println!(
        "Usage: cargo run --release --bin latency -- [options]\n\
\n\
Options:\n\
  --probe NAME              Probe to run, default syscall_getpid\n\
  --samples N               Measurement samples, default 100000\n\
  --warmup-samples N        Warmup samples, default 10000\n\
  --batch N                 Operations per sample; defaults per probe\n\
  --buckets N               Histogram buckets, default 32\n\
  --size BYTES              Probe-specific size, accepts 64m/8k/1g\n\
  --csv PATH                Write raw and adjusted per-sample CSV\n\
  --list                    Show available probes\n\
  -h, --help                Show this help\n\
\n\
Examples:\n\
  cargo run --release --bin latency -- --probe syscall_getpid --csv target/getpid.csv\n\
  cargo run --release --bin latency -- --probe memory_random --size 256m --batch 1024\n\
  cargo run --release --bin latency -- --probe tcp_echo --size 1 --samples 50000"
    );
}

fn print_probe_list() {
    println!("available probes:");
    println!("  syscall_getpid  libc getpid latency");
    println!("  stat_tmp        metadata lookup for /tmp");
    println!("  memory_random   dependent random memory load latency");
    println!("  tcp_echo        localhost TCP echo round-trip latency");
}

fn make_probe(config: &Config) -> Result<Box<dyn Probe>, Box<dyn Error>> {
    match config.probe.as_str() {
        "syscall_getpid" | "getpid" => Ok(Box::new(SyscallGetpidProbe)),
        "stat_tmp" | "stat" => Ok(Box::new(StatTmpProbe)),
        "memory_random" | "random_memory" => Ok(Box::new(MemoryRandomProbe::new(
            config.size_bytes.unwrap_or(64 * 1024 * 1024),
        )?)),
        "tcp_echo" => Ok(Box::new(TcpEchoProbe::new(config.size_bytes.unwrap_or(1))?)),
        name => Err(format!("unknown probe: {}", name).into()),
    }
}

fn measure_samples<P: Probe + ?Sized>(
    probe: &mut P,
    warmup_samples: usize,
    samples: usize,
    batch: usize,
) -> Vec<u64> {
    for _ in 0..warmup_samples {
        for _ in 0..batch {
            probe.run_once();
        }
    }

    let mut timings = Vec::with_capacity(samples);
    for _ in 0..samples {
        let started = Instant::now();
        for _ in 0..batch {
            probe.run_once();
        }
        let elapsed = started.elapsed().as_nanos() as u64;
        timings.push(elapsed / batch as u64);
    }
    timings
}

struct NoopProbe;

impl Probe for NoopProbe {
    fn name(&self) -> &'static str {
        "noop"
    }

    fn default_batch(&self) -> usize {
        1_000
    }

    fn run_once(&mut self) {
        black_box(());
    }
}

struct SyscallGetpidProbe;

impl Probe for SyscallGetpidProbe {
    fn name(&self) -> &'static str {
        "syscall_getpid"
    }

    fn default_batch(&self) -> usize {
        256
    }

    fn run_once(&mut self) {
        unsafe {
            black_box(libc::getpid());
        }
    }
}

struct StatTmpProbe;

impl Probe for StatTmpProbe {
    fn name(&self) -> &'static str {
        "stat_tmp"
    }

    fn default_batch(&self) -> usize {
        1
    }

    fn run_once(&mut self) {
        let metadata = fs::metadata("/tmp").expect("stat /tmp");
        black_box(metadata.len());
    }
}

struct MemoryRandomProbe {
    pages: Vec<usize>,
    index: usize,
}

impl MemoryRandomProbe {
    fn new(size_bytes: usize) -> Result<Self, Box<dyn Error>> {
        let entries = size_bytes / std::mem::size_of::<usize>();
        if entries < 2 {
            return Err("memory_random --size must hold at least two usize values".into());
        }

        let mut pages: Vec<usize> = (0..entries).collect();
        shuffle(&mut pages);

        let mut next = vec![0usize; entries];
        for window in pages.windows(2) {
            next[window[0]] = window[1];
        }
        next[*pages.last().unwrap()] = pages[0];

        Ok(Self {
            pages: next,
            index: pages[0],
        })
    }
}

impl Probe for MemoryRandomProbe {
    fn name(&self) -> &'static str {
        "memory_random"
    }

    fn default_batch(&self) -> usize {
        1024
    }

    fn run_once(&mut self) {
        self.index = self.pages[self.index];
        black_box(self.index);
    }
}

struct TcpEchoProbe {
    stream: TcpStream,
    request: Vec<u8>,
    response: Vec<u8>,
}

impl TcpEchoProbe {
    fn new(payload_bytes: usize) -> Result<Self, Box<dyn Error>> {
        if payload_bytes == 0 {
            return Err("tcp_echo --size must be greater than zero".into());
        }

        let listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;

        thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut buffer = vec![0u8; payload_bytes];
            loop {
                if stream.read_exact(&mut buffer).is_err() {
                    return;
                }
                if stream.write_all(&buffer).is_err() {
                    return;
                }
            }
        });

        let stream = TcpStream::connect(addr)?;
        stream.set_nodelay(true)?;

        Ok(Self {
            stream,
            request: vec![0xA5; payload_bytes],
            response: vec![0; payload_bytes],
        })
    }
}

impl Probe for TcpEchoProbe {
    fn name(&self) -> &'static str {
        "tcp_echo"
    }

    fn default_batch(&self) -> usize {
        1
    }

    fn run_once(&mut self) {
        self.stream.write_all(&self.request).expect("tcp write");
        self.stream
            .read_exact(&mut self.response)
            .expect("tcp read");
        black_box(self.response[0]);
    }
}

fn sorted(samples: &[u64]) -> Vec<u64> {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    sorted
}

fn print_summary(sorted_samples: &[u64]) {
    let mean = mean(sorted_samples);
    let stddev = stddev(sorted_samples, mean);

    println!("summary:");
    println!(
        "  min:   {}",
        format_latency(percentile(sorted_samples, 0.00))
    );
    println!(
        "  p50:   {}",
        format_latency(percentile(sorted_samples, 0.50))
    );
    println!(
        "  p90:   {}",
        format_latency(percentile(sorted_samples, 0.90))
    );
    println!(
        "  p95:   {}",
        format_latency(percentile(sorted_samples, 0.95))
    );
    println!(
        "  p99:   {}",
        format_latency(percentile(sorted_samples, 0.99))
    );
    println!(
        "  p99.9: {}",
        format_latency(percentile(sorted_samples, 0.999))
    );
    println!(
        "  max:   {}",
        format_latency(percentile(sorted_samples, 1.00))
    );
    println!("  mean:  {}", format_latency(mean.round() as u64));
    println!("  stdev: {}", format_latency(stddev.round() as u64));
}

fn percentile(sorted_samples: &[u64], percentile: f64) -> u64 {
    if sorted_samples.is_empty() {
        return 0;
    }

    let last = sorted_samples.len() - 1;
    let index = ((last as f64) * percentile).round() as usize;
    sorted_samples[index.min(last)]
}

fn mean(samples: &[u64]) -> f64 {
    samples.iter().sum::<u64>() as f64 / samples.len() as f64
}

fn stddev(samples: &[u64], mean: f64) -> f64 {
    let variance = samples
        .iter()
        .map(|sample| {
            let distance = *sample as f64 - mean;
            distance * distance
        })
        .sum::<f64>()
        / samples.len() as f64;
    variance.sqrt()
}

fn print_histogram(sorted_samples: &[u64], buckets: usize) {
    let min = sorted_samples[0];
    let max = *sorted_samples.last().unwrap();
    println!("histogram:");

    if min == max {
        println!(
            "  {:>12} - {:>12} | {:>8} {}",
            format_latency(min),
            format_latency(max),
            sorted_samples.len(),
            "#".repeat(50)
        );
        return;
    }

    let use_log_scale = min > 0 && (max / min.max(1)) > 100;
    let mut counts = vec![0usize; buckets];

    if use_log_scale {
        let min_log = (min.max(1) as f64).log2();
        let max_log = (max.max(1) as f64).log2();
        let step = (max_log - min_log) / buckets as f64;

        for sample in sorted_samples {
            let idx = if step == 0.0 {
                0
            } else {
                ((((*sample).max(1) as f64).log2() - min_log) / step) as usize
            };
            counts[idx.min(buckets - 1)] += 1;
        }

        print_buckets(&counts, |idx| {
            let low = 2f64.powf(min_log + step * idx as f64).round() as u64;
            let high = 2f64.powf(min_log + step * (idx + 1) as f64).round() as u64;
            (low, high.max(low))
        });
    } else {
        let width = ((max - min + 1) as f64 / buckets as f64).ceil() as u64;
        for sample in sorted_samples {
            let idx = ((sample - min) / width) as usize;
            counts[idx.min(buckets - 1)] += 1;
        }

        print_buckets(&counts, |idx| {
            let low = min + width * idx as u64;
            let high = low + width - 1;
            (low, high.min(max))
        });
    }
}

fn print_buckets<F>(counts: &[usize], mut range: F)
where
    F: FnMut(usize) -> (u64, u64),
{
    let max_count = counts.iter().copied().max().unwrap_or(1).max(1);
    for (idx, count) in counts.iter().enumerate() {
        if *count == 0 {
            continue;
        }

        let (low, high) = range(idx);
        let bar_len = ((*count as f64 / max_count as f64) * 50.0).round() as usize;
        println!(
            "  {:>12} - {:>12} | {:>8} {}",
            format_latency(low),
            format_latency(high),
            count,
            "#".repeat(bar_len.max(1))
        );
    }
}

fn write_csv(
    path: &str,
    raw_samples: &[u64],
    adjusted_samples: &[u64],
) -> Result<(), Box<dyn Error>> {
    let path = Path::new(path);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    let mut file = fs::File::create(path)?;
    writeln!(file, "sample,raw_ns,adjusted_ns")?;
    for (idx, (raw, adjusted)) in raw_samples.iter().zip(adjusted_samples).enumerate() {
        writeln!(file, "{},{},{}", idx, raw, adjusted)?;
    }
    Ok(())
}

fn format_latency(nanos: u64) -> String {
    if nanos < 1_000 {
        format!("{} ns", nanos)
    } else if nanos < 1_000_000 {
        format!("{:.2} us", nanos as f64 / 1_000.0)
    } else if nanos < 1_000_000_000 {
        format!("{:.2} ms", nanos as f64 / 1_000_000.0)
    } else {
        format!("{:.2} s", nanos as f64 / 1_000_000_000.0)
    }
}

fn parse_size(input: &str) -> Result<usize, Box<dyn Error>> {
    let input = input.trim().to_ascii_lowercase();
    let (number, multiplier) = if let Some(number) = input.strip_suffix("kib") {
        (number, 1024usize)
    } else if let Some(number) = input.strip_suffix("kb") {
        (number, 1024usize)
    } else if let Some(number) = input.strip_suffix('k') {
        (number, 1024usize)
    } else if let Some(number) = input.strip_suffix("mib") {
        (number, 1024usize * 1024)
    } else if let Some(number) = input.strip_suffix("mb") {
        (number, 1024usize * 1024)
    } else if let Some(number) = input.strip_suffix('m') {
        (number, 1024usize * 1024)
    } else if let Some(number) = input.strip_suffix("gib") {
        (number, 1024usize * 1024 * 1024)
    } else if let Some(number) = input.strip_suffix("gb") {
        (number, 1024usize * 1024 * 1024)
    } else if let Some(number) = input.strip_suffix('g') {
        (number, 1024usize * 1024 * 1024)
    } else {
        (input.as_str(), 1usize)
    };

    let parsed = number.parse::<usize>()?;
    Ok(parsed * multiplier)
}

fn shuffle(values: &mut [usize]) {
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    for i in (1..values.len()).rev() {
        state = xorshift64(state);
        values.swap(i, state as usize % (i + 1));
    }
}

fn xorshift64(mut state: u64) -> u64 {
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;
    state
}

fn print_context() {
    println!("context:");
    println!("  os: {} {}", env::consts::OS, env::consts::ARCH);
    if let Ok(hostname) = command_output("hostname", &[]) {
        println!("  host: {}", hostname);
    }
    if let Some(cpu) = cpu_model() {
        println!("  cpu: {}", cpu);
    }
}

fn cpu_model() -> Option<String> {
    if let Ok(cpuinfo) = fs::read_to_string("/proc/cpuinfo") {
        for line in cpuinfo.lines() {
            if let Some(model) = line.strip_prefix("model name") {
                return model
                    .split_once(':')
                    .map(|(_, value)| value.trim().to_string());
            }
            if let Some(model) = line.strip_prefix("Hardware") {
                return model
                    .split_once(':')
                    .map(|(_, value)| value.trim().to_string());
            }
        }
    }

    command_output("sysctl", &["-n", "machdep.cpu.brand_string"]).ok()
}

fn command_output(command: &str, args: &[&str]) -> Result<String, Box<dyn Error>> {
    let output = Command::new(command).args(args).output()?;
    if !output.status.success() {
        return Err(format!("{} failed", command).into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[inline(never)]
fn black_box<T>(dummy: T) -> T {
    unsafe {
        let ret = std::ptr::read_volatile(&dummy);
        std::mem::forget(dummy);
        ret
    }
}
