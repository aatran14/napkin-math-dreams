# Napkin Math Dreams

Daily benchmark measurements on real hardware. Inspired by
[sirupsen/napkin-math](https://github.com/sirupsen/napkin-math).

## Numbers

Measured on Prince.local (Apple M4 Pro), May 25, 2026. Baseline config (stock
kernel defaults, no tuning).

| Operation                           | Latency     | Throughput | 1 MiB  | 1 GiB  |
| ----------------------------------- | ----------- | ---------- | ------ | ------ |
| Sequential Memory R/W (64 bytes)    |             |            |        |        |
| ├ Single Thread                     | 49 ns       | 1.2 GiB/s  | 800 μs | 800 ms |
| ├ Threaded                          |             | 83 GiB/s   | 12 μs  | 12 ms  |
| Hashing, non-crypto (64 bytes)      | 34 ns       | 1.7 GiB/s  | 600 μs | 600 ms |
| Random Memory R/W (64 bytes)        | 55 ns       | 1.1 GiB/s  | 900 μs | 900 ms |
| System Call                         | 18 ns       | N/A        | N/A    | N/A    |
| Hashing, crypto-safe (64 bytes)     | 251 ns      | 243 MiB/s  | 4 ms   | 4s     |
| Sequential SSD Read (8 KiB)         | 802 ns      | 9.5 GiB/s  | 100 μs | 100 ms |
| Context Switch                      | 2.3 μs      | N/A        | N/A    | N/A    |
| Sequential SSD Write, -fsync (8KiB) | 4.7 μs      | 1.6 GiB/s  | 600 μs | 600 ms |
| TCP Echo Server (32 KiB)            | 23 μs       | 1.3 GiB/s  | 700 μs | 700 ms |
| Random SSD Read (8 KiB)             | 158 μs      | 49 MiB/s   | 20 ms  | 20s    |
| Sequential SSD Write, +fsync (8KiB) | 4.1 ms      | 1.9 MiB/s  | 500 ms | 500s   |
| Fast Serialization (bincode)        | 33 ns       | 4.4 GiB/s  | 200 μs | 200 ms |
| Fast Deserialization (bincode)      | 56 ns       | 2.6 GiB/s  | 400 μs | 400 ms |
| Serialization (JSON)                | 389 ns      | 350 MiB/s  | 3 ms   | 3s     |
| Deserialization (JSON)              | 230 ns      | 593 MiB/s  | 2 ms   | 2s     |
| Sorting (64-bit integers)           | N/A         | 929 MiB/s  | 1 ms   | 1s     |
| Compression (LZ4)                   | N/A         | 29 GiB/s   | 30 μs  | 30 ms  |
| Decompression (LZ4)                 | N/A         | 11.8 GiB/s | 80 μs  | 80 ms  |

Raw data: [data/dead.csv](data/dead.csv)

## Roadmap

Target machines — one per architecture per cloud.

| Cloud | Intel | AMD | ARM |
| ----- | ----- | --- | --- |
| GCP   | [ ] C4  | [ ] C4D  | [ ] C4A  |
| AWS   | [ ] C7i | [ ] C7a  | [ ] C8g  |
| Azure | [ ] Dv6 | [ ] Dav6 | [ ] Dpv6 |

## Running

```
cargo run --release --bin daily
```

Set `NAPKIN_MACHINE` and `NAPKIN_CONFIG` env vars to label the run.
Results append to `data/dead.csv`.

For cloud VMs, see [machines/README.md](machines/README.md).
