use crate::benchmarks::{bench, black_box, Measurement};
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};

fn bench_file() -> String {
    std::env::var("NAPKIN_BENCH_FILE").unwrap_or_else(|_| "/tmp/napkin_daily.bin".into())
}

// Sequential SSD Read (8 KiB)
// README: ~1 μs latency, ~8 GiB/s throughput
pub fn seq_read() -> Measurement {
    let path = bench_file();
    let buf_size: usize = 8 * 1024;

    // write a 1 GiB file
    {
        let mut f = OpenOptions::new().create(true).write(true).truncate(true).open(&path).unwrap();
        let zeros = vec![0u8; 1024 * 1024];
        for _ in 0..1024 { f.write_all(&zeros).unwrap(); }
        f.sync_data().unwrap();
    }

    drop_caches(&path);

    let mut f = OpenOptions::new().read(true).open(&path).unwrap();
    let mut buf = vec![0u8; buf_size];

    let m = bench("ssd_read_seq", buf_size, 5, || {
        let n = f.read(&mut buf).unwrap();
        if n < buf_size { f.seek(SeekFrom::Start(0)).unwrap(); }
        black_box(buf[0]);
    });

    let _ = fs::remove_file(&path);
    m
}

// Sequential SSD Write, -fsync (8 KiB)
// README: ~2 μs latency, ~3 GiB/s throughput
pub fn seq_write_no_fsync() -> Measurement {
    let path = bench_file();
    let buf_size: usize = 8 * 1024;
    let data: Vec<u8> = (0..buf_size).map(|i| i as u8).collect();

    let mut f = OpenOptions::new().create(true).write(true).truncate(true).open(&path).unwrap();

    let m = bench("ssd_write_no_fsync", buf_size, 5, || {
        f.write_all(&data).unwrap();
    });

    let _ = fs::remove_file(&path);
    m
}

// Sequential SSD Write, +fsync (8 KiB)
// README: ~300 μs latency, ~30 MiB/s throughput
pub fn seq_write_fsync() -> Measurement {
    let path = bench_file();
    let buf_size: usize = 8 * 1024;
    let data: Vec<u8> = (0..buf_size).map(|i| i as u8).collect();

    let mut f = OpenOptions::new().create(true).write(true).truncate(true).open(&path).unwrap();

    let m = bench("ssd_write_fsync", buf_size, 5, || {
        f.write_all(&data).unwrap();
        f.sync_data().unwrap();
    });

    let _ = fs::remove_file(&path);
    m
}

// Random SSD Read (8 KiB)
// README: ~100 μs latency, ~70 MiB/s throughput
pub fn random_read() -> Measurement {
    let path = bench_file();
    let buf_size: usize = 8 * 1024;
    let file_size: usize = 8 * 1024 * 1024 * 1024; // 8 GiB
    let page_size = 4096usize;

    // write a large file
    {
        let mut f = OpenOptions::new().create(true).write(true).truncate(true).open(&path).unwrap();
        let zeros = vec![0u8; 1024 * 1024];
        for _ in 0..(file_size / (1024 * 1024)) { f.write_all(&zeros).unwrap(); }
        f.sync_data().unwrap();
    }

    drop_caches(&path);

    let num_pages = file_size / page_size;
    let mut offsets: Vec<u64> = (0..num_pages).map(|i| (i * page_size) as u64).collect();
    {
        use rand::seq::SliceRandom;
        offsets.shuffle(&mut rand::thread_rng());
    }

    let mut f = OpenOptions::new().read(true).open(&path).unwrap();
    let mut buf = vec![0u8; buf_size];
    let mut idx = 0usize;

    let m = bench("ssd_read_random", buf_size, 5, || {
        f.seek(SeekFrom::Start(offsets[idx])).unwrap();
        f.read_exact(&mut buf).unwrap();
        black_box(buf[0]);
        idx += 1;
        if idx >= offsets.len() { idx = 0; }
    });

    let _ = fs::remove_file(&path);
    m
}

pub fn run() -> Vec<Measurement> {
    vec![
        seq_read(),
        seq_write_no_fsync(),
        seq_write_fsync(),
        random_read(),
    ]
}

fn drop_caches(_path: &str) {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::io::AsRawFd;
        if let Ok(f) = fs::File::open(_path) {
            unsafe { libc::posix_fadvise(f.as_raw_fd(), 0, 0, libc::POSIX_FADV_DONTNEED); }
        }
        let _ = fs::write("/proc/sys/vm/drop_caches", "3");
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("sudo").arg("purge").output();
    }
}
