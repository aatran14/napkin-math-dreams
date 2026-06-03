//! README benchmarks, run in a practical order:
//! memory first, then cheap CPU/kernel, then disk and network last.

use crate::benchmarks::{compression, hash, memory, network, serialization, sort, ssd, syscall, Measurement};

pub struct Section {
    pub name: &'static str,
    pub measurements: Vec<Measurement>,
}

pub fn sections() -> Vec<Section> {
    vec![
        Section {
            name: "memory",
            measurements: vec![
                memory::seq_read_single(),
                memory::seq_read_threaded(),
                memory::seq_write_single(),
                memory::seq_write_threaded(),
                memory::random_read(),
            ],
        },
        Section {
            name: "syscall",
            measurements: vec![syscall::getpid(), syscall::context_switch()],
        },
        Section {
            name: "hash",
            measurements: vec![hash::non_crypto(), hash::crypto()],
        },
        Section {
            name: "cpu",
            measurements: vec![
                sort::sort_u64(),
                compression::compress(),
                compression::decompress(),
            ],
        },
        Section {
            name: "serialization",
            measurements: vec![
                serialization::fast_serialize(),
                serialization::fast_deserialize(),
                serialization::slow_serialize(),
                serialization::slow_deserialize(),
            ],
        },
        Section {
            name: "ssd",
            measurements: vec![
                ssd::seq_read(),
                ssd::seq_write_no_fsync(),
                ssd::seq_write_fsync(),
                ssd::random_read(),
            ],
        },
        Section {
            name: "network",
            measurements: vec![network::tcp_echo()],
        },
    ]
}

pub fn run() -> Vec<Measurement> {
    sections()
        .into_iter()
        .flat_map(|s| s.measurements)
        .collect()
}
