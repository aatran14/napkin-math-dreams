use crate::benchmarks::{bench, black_box, Measurement};
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};

fn bench_file() -> String {
    std::env::var("NAPKIN_BENCH_FILE").unwrap_or_else(|_| "/tmp/napkin_daily.bin".into())
}

// Sequential SSD read (8 KiB).
// Kernel prefetches ahead, so this is best-case disk read.
pub fn seq_read() -> Measurement {
    let path = bench_file();
    let buf_size: usize = 8 * 1024;

    // write a 1 GiB test file
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

// Sequential SSD write without fsync (8 KiB).
// Writes go to the kernel page cache and return immediately. Not durable, a crash loses this data.
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

// Sequential SSD write with fsync (8 KiB).
// fsync() waits for the SSD's flash cells to actually write. ~100x slower than without.
// This is what databases pay for every commit.
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

// Random SSD read (8 KiB).
// Shuffled offsets defeat the SSD's read-ahead.
// 8 GiB file is larger than RAM so reads hit the actual SSD, not page cache.
pub fn random_read() -> Measurement {
    let path = bench_file();
    let buf_size: usize = 8 * 1024;
    let file_size: usize = 8 * 1024 * 1024 * 1024; // 8 GiB
    let page_size = 4096usize;

    // 8 GiB file, larger than RAM on most machines, to avoid page cache hits
    {
        let mut f = OpenOptions::new().create(true).write(true).truncate(true).open(&path).unwrap();
        let zeros = vec![0u8; 1024 * 1024];
        for _ in 0..(file_size / (1024 * 1024)) { f.write_all(&zeros).unwrap(); }
        f.sync_data().unwrap();
    }

    drop_caches(&path);

    // stop one page short: an 8 KiB read at the final 4 KiB page would run past EOF
    let num_pages = file_size / page_size - (buf_size / page_size);
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
