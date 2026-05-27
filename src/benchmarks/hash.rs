use crate::benchmarks::{bench, black_box, Measurement};
use sha2::{Sha256, Digest};
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;

// CRC32:
// Has hardware acceleration on modern CPUs (Intel SSE4.2, ARM CRC extension)
// I suspect that if this is slow, the hardware instruction isn't being used
pub fn non_crypto() -> Measurement {
    let data: Vec<u8> = (0..64).map(|i| i as u8).collect();

    bench("hash_non_crypto", 64, 5, || {
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(&data);
        black_box(hasher.finalize());
    })
}

// SHA-256
// Some CPUs have SHA extensions (Intel SHA-NI, ARM SHA2) that speed this up significantly.
pub fn crypto() -> Measurement {
    let data: Vec<u8> = (0..64).map(|i| i as u8).collect();

    bench("hash_crypto", 64, 5, || {
        black_box(Sha256::digest(&data));
    })
}

pub fn siphash() -> Measurement {
    let data: Vec<u8> = (0..64).map(|i| i as u8).collect();

    bench("hash_siphash", 64, 5, || {
        let mut hasher = DefaultHasher::new();
        hasher.write(&data);
        black_box(hasher.finish());
    })
}

pub fn run() -> Vec<Measurement> {
    vec![non_crypto(), crypto(), siphash()]
}
