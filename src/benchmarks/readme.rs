//! README benchmarks, run in a practical order:
//! memory first, then cheap CPU/kernel, then disk and network last.
//!
//! TODO list: flip `false` -> `true` to turn a benchmark on. These ship to the
//! fleet VMs with the code, so editing here + pushing `master` is all it takes.
//! A section with everything off is dropped entirely.

use crate::benchmarks::{compression, hash, memory, network, serialization, sort, ssd, syscall, Measurement};

pub struct Section {
    pub name: &'static str,
    pub measurements: Vec<Measurement>,
}

// (enabled, label, benchmark) — label is just inline documentation for the list.
type Bench = (bool, &'static str, fn() -> Measurement);

fn section(name: &'static str, benches: &[Bench]) -> Section {
    Section {
        name,
        measurements: benches.iter().filter(|(on, _, _)| *on).map(|(_, _, f)| f()).collect(),
    }
}

pub fn sections() -> Vec<Section> {
    let all = vec![
        section("memory", &[
            (true,  "seq_read_single",     memory::seq_read_single),
            (true,  "seq_read_threaded",   memory::seq_read_threaded),
            (true,  "seq_write_single",    memory::seq_write_single),
            (true,  "seq_write_threaded",  memory::seq_write_threaded),
            (true,  "random_read",         memory::random_read),
            (true,  "random_write",        memory::random_write),
        ]),
        section("syscall", &[
            (false, "getpid",              syscall::getpid),
            (false, "context_switch",      syscall::context_switch),
        ]),
        section("hash", &[
            (false, "non_crypto",          hash::non_crypto),
            (false, "crypto",              hash::crypto),
        ]),
        section("cpu", &[
            (false, "sort_u64",            sort::sort_u64),
            (false, "compress",            compression::compress),
            (false, "decompress",          compression::decompress),
        ]),
        section("serialization", &[
            (false, "fast_serialize",      serialization::fast_serialize),
            (false, "fast_deserialize",    serialization::fast_deserialize),
            (false, "slow_serialize",      serialization::slow_serialize),
            (false, "slow_deserialize",    serialization::slow_deserialize),
        ]),
        section("ssd", &[
            (true,  "seq_read",            ssd::seq_read),
            (true,  "seq_write_no_fsync",  ssd::seq_write_no_fsync),
            (true,  "seq_write_fsync",     ssd::seq_write_fsync),
            (false, "random_read",         ssd::random_read),
        ]),
        section("network", &[
            (false, "tcp_echo",            network::tcp_echo),
        ]),
    ];
    all.into_iter().filter(|s| !s.measurements.is_empty()).collect()
}

pub fn run() -> Vec<Measurement> {
    sections()
        .into_iter()
        .flat_map(|s| s.measurements)
        .collect()
}
