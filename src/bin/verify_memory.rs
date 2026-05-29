use napkin_math::benchmarks::black_box;

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("verify_memory requires Linux (perf_event_open)");
    std::process::exit(1);
}

#[cfg(target_os = "linux")]
fn main() {
    use perf_event::events::Hardware;
    use perf_event::Builder;
    use std::time::Instant;

    let size_bytes: usize = 1024 * 1024 * 1024; // 1 GiB
    let n = size_bytes / 8;
    let vec: Vec<u64> = (0..n).map(|i| i as u64).collect();

    // Flush every cache line
    unsafe { flush_cache(&vec) };

    let mut builder = Builder::new();
    builder.kind(Hardware::CACHE_MISSES);
    builder.exclude_kernel(true);
    builder.exclude_hv(true);
    let mut counter = builder.build()
        .expect("failed to create perf counter — run: sudo sysctl -w kernel.perf_event_paranoid=-1");

    // Single pass with counter enabled
    counter.reset().unwrap();
    counter.enable().unwrap();
    let t = Instant::now();
    let sum = memory_read_vectorized(&vec);
    let elapsed = t.elapsed();
    counter.disable().unwrap();

    let misses = counter.read().unwrap();
    let cache_lines = size_bytes / 64;
    let miss_pct = misses as f64 / cache_lines as f64 * 100.0;
    let throughput_gibs = size_bytes as f64 / elapsed.as_secs_f64() / (1024.0 * 1024.0 * 1024.0);

    eprintln!("seq_mem_read_single verification");
    eprintln!("  buffer:       1 GiB");
    eprintln!("  elapsed:      {:.2} ms", elapsed.as_secs_f64() * 1000.0);
    eprintln!("  throughput:   {:.2} GiB/s", throughput_gibs);
    eprintln!("  cache misses: {}", misses);
    eprintln!("  cache lines:  {}", cache_lines);
    eprintln!("  miss rate:    {:.2}%", miss_pct);
    eprintln!();

    if miss_pct > 90.0 {
        eprintln!("  PASS — reading from RAM, not cache");
    } else {
        eprintln!("  FAIL — {:.0}% of reads hit cache, not RAM", 100.0 - miss_pct);
    }

    black_box(sum);
}

#[cfg(target_os = "linux")]
unsafe fn flush_cache(data: &[u64]) {
    let ptr = data.as_ptr() as *const u8;
    let len = data.len() * 8;
    let mut offset = 0usize;

    while offset < len {
        #[cfg(target_arch = "x86_64")]
        std::arch::asm!(
            "clflushopt [{addr}]",
            addr = in(reg) ptr.add(offset),
            options(nostack, preserves_flags),
        );

        #[cfg(target_arch = "aarch64")]
        std::arch::asm!(
            "dc civac, {addr}",
            addr = in(reg) ptr.add(offset),
            options(nostack, preserves_flags),
        );

        offset += 64;
    }

    #[cfg(target_arch = "x86_64")]
    std::arch::asm!("mfence", options(nostack, preserves_flags));

    #[cfg(target_arch = "aarch64")]
    std::arch::asm!("dsb sy", options(nostack, preserves_flags));
}

#[cfg(target_os = "linux")]
#[inline(never)]
fn memory_read_vectorized(vec: &[u64]) -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            unsafe { return memory_read_avx2(vec); }
        }
    }

    let mut sum = 0u64;
    for value in vec {
        sum = sum.wrapping_add(*value);
    }
    sum
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn memory_read_avx2(vec: &[u64]) -> u64 {
    use std::arch::x86_64::*;

    let mut acc0 = _mm256_setzero_si256();
    let mut acc1 = _mm256_setzero_si256();
    let mut acc2 = _mm256_setzero_si256();
    let mut acc3 = _mm256_setzero_si256();
    let mut i = 0usize;
    let ptr = vec.as_ptr();

    while i + 16 <= vec.len() {
        acc0 = _mm256_add_epi64(acc0, _mm256_loadu_si256(ptr.add(i) as *const __m256i));
        acc1 = _mm256_add_epi64(acc1, _mm256_loadu_si256(ptr.add(i + 4) as *const __m256i));
        acc2 = _mm256_add_epi64(acc2, _mm256_loadu_si256(ptr.add(i + 8) as *const __m256i));
        acc3 = _mm256_add_epi64(acc3, _mm256_loadu_si256(ptr.add(i + 12) as *const __m256i));
        i += 16;
    }

    acc0 = _mm256_add_epi64(acc0, acc1);
    acc2 = _mm256_add_epi64(acc2, acc3);
    acc0 = _mm256_add_epi64(acc0, acc2);

    let mut lanes = [0u64; 4];
    _mm256_storeu_si256(lanes.as_mut_ptr() as *mut __m256i, acc0);

    let mut sum = lanes.iter().copied().sum::<u64>();
    while i < vec.len() {
        sum = sum.wrapping_add(*ptr.add(i));
        i += 1;
    }

    sum
}
