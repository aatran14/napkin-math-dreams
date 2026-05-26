use crate::benchmarks::{bench, black_box, Measurement};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct Record {
    id: u64,
    name: String,
    values: Vec<f64>,
    active: bool,
}

fn make_record() -> Record {
    Record {
        id: 123456789,
        name: "benchmark test record with a reasonable length name".into(),
        values: vec![1.1, 2.2, 3.3, 4.4, 5.5, 6.6, 7.7, 8.8, 9.9, 10.0],
        active: true,
    }
}

// Fast Serialization (e.g. bincode, flatbuffers, cap'n proto)
// README: ~1 GiB/s throughput
pub fn fast_serialize() -> Measurement {
    let record = make_record();
    let size = bincode::serialized_size(&record).unwrap() as usize;

    bench("serialization_fast", size, 5, || {
        black_box(bincode::serialize(&record).unwrap());
    })
}

// Fast Deserialization
// README: ~1 GiB/s throughput
pub fn fast_deserialize() -> Measurement {
    let record = make_record();
    let encoded = bincode::serialize(&record).unwrap();
    let size = encoded.len();

    bench("deserialization_fast", size, 5, || {
        black_box(bincode::deserialize::<Record>(&encoded).unwrap());
    })
}

// Serialization (e.g. JSON, standard protobuf)
// README: ~100 MiB/s throughput
pub fn slow_serialize() -> Measurement {
    let record = make_record();
    let size = serde_json::to_vec(&record).unwrap().len();

    bench("serialization", size, 5, || {
        black_box(serde_json::to_vec(&record).unwrap());
    })
}

// Deserialization (e.g. JSON)
// README: ~100 MiB/s throughput
pub fn slow_deserialize() -> Measurement {
    let record = make_record();
    let encoded = serde_json::to_vec(&record).unwrap();
    let size = encoded.len();

    bench("deserialization", size, 5, || {
        black_box(serde_json::from_slice::<Record>(&encoded).unwrap());
    })
}

pub fn run() -> Vec<Measurement> {
    vec![
        fast_serialize(),
        fast_deserialize(),
        slow_serialize(),
        slow_deserialize(),
    ]
}
