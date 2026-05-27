use crate::benchmarks::{bench, black_box, Measurement};

// LZ4 compression (1 MiB).
// LZ4 is the standard fast compressor used in databases (ClickHouse, RocksDB), ZFS, network protocols.
pub fn compress() -> Measurement {
    let size: usize = 1024 * 1024; // 1 MiB
    let data = make_compressible_data(size);
    let mut output = vec![0u8; lz4_flex::block::get_maximum_output_size(size)];

    bench("compression", size, 5, || {
        black_box(lz4_flex::block::compress_into(&data, &mut output).unwrap());
    })
}

// LZ4 decompression (1 MiB).
// Decompression is typically faster than compression.
pub fn decompress() -> Measurement {
    let size: usize = 1024 * 1024; // 1 MiB
    let data = make_compressible_data(size);
    let compressed = lz4_flex::block::compress(&data);
    let mut output = vec![0u8; size];

    bench("decompression", size, 5, || {
        black_box(lz4_flex::block::decompress_into(&compressed, &mut output).unwrap());
    })
}

pub fn run() -> Vec<Measurement> {
    vec![compress(), decompress()]
}

fn make_compressible_data(size: usize) -> Vec<u8> {
    // english-like text has ~2-4x compression ratio
    let words = b"the quick brown fox jumps over the lazy dog ";
    let mut data = Vec::with_capacity(size);
    while data.len() < size {
        let remaining = size - data.len();
        let chunk = if remaining < words.len() { &words[..remaining] } else { &words[..] };
        data.extend_from_slice(chunk);
    }
    data
}
