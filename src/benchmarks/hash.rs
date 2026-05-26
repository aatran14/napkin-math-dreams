use crate::benchmarks::{bench, black_box, Measurement};
use sha2::{Sha256, Digest};

// Hashing, not crypto-safe (64 bytes)
// README: ~10 ns latency, ~5 GiB/s throughput
pub fn non_crypto() -> Measurement {
    let data: Vec<u8> = (0..64).map(|i| i as u8).collect();

    bench("hash_non_crypto", 64, 5, || {
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(&data);
        black_box(hasher.finalize());
    })
}

// Hashing, crypto-safe (64 bytes)
// README: ~100 ns latency, ~1 GiB/s throughput
pub fn crypto() -> Measurement {
    let data: Vec<u8> = (0..64).map(|i| i as u8).collect();

    bench("hash_crypto", 64, 5, || {
        black_box(Sha256::digest(&data));
    })
}

pub fn run() -> Vec<Measurement> {
    vec![non_crypto(), crypto()]
}
