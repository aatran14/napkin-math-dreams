# Napkin Math Dreams

Daily benchmark measurements on real hardware. Forked from
[sirupsen/napkin-math](https://github.com/sirupsen/napkin-math). 

The make the best use of this repo, it's reccomended you look at Simon's first. It's there where you will get a comprehensive introduction to the impetus of this dream scenario.

## Numbers

Master raw data: [data/dead.csv](data/dead.csv)

## Goals
[x] What we are optimizing for right now
[] What we're definitely not optimizing for

What we are optimizing for right now is writing code that we can "fire-and-forget". It is the nature of benchmarking that you can p9999 hack, but in the interest of simplicity, the goal of this project is to arrive there at steady state. 

Optimize for simplicity on getting your first 9, then build the next method.

What we are definitely not optimizing for is the UI. We keep it bare bones to reduce distractions. 

What we are definitely not optimizing for is writing any comparative descriptions of these benchmarks. We aim to be faithful to the machines. In turn, we leave analyses up to humans. The crux of benchmarking is that it is often malpracticed. Prune bullshit wherever possible.

The reason for these choices is that we get a more intimiate understanding of machines by working from naive understandings and making large improvements with first principles. Once the fleet arives to its p99, then the ball is in the course of the communtiy to keep hacking 9s. For now, we focus on one row of the benchmark at a time. This is the first deadlift.


## Roadmap

Target machines: one per architecture per cloud. Ideally is more transparent about their cores and specs, but currently focused on bolting them onto the fleet.

| Cloud | Intel | AMD | ARM |
| ----- | ----- | --- | --- |
| GCP   | C4  | C4D  | C4A  |
| AWS   | C7i | C7a  | C8g  |
| Azure | Dv6 | Dav6 | Dpv6 |

- [x] GCP C4 (Intel) (24 cores)
- [x] GCP C4D (AMD) (24 cores)
- [x] GCP C4A (ARM) (24 cores)
- [x] AWS C7i (Intel) (48 cores)
- [x] AWS C7a (AMD)
- [x] AWS C7g (ARM)
- [ ] Azure Dv6 (Intel)
- [ ] Azure Dav6 (AMD)
- [ ] Azure Dpv6 (ARM)

## Running

Run all benchmarks and print results to terminal:

```
cargo run --release --bin readme
```

Run memory benchmarks:

```
cargo run --release --bin memory
```

Daily run (appends to `data/dead.csv`):

```
cargo run --release --bin daily
```

Run on a cloud VM (requires `gcloud` or `aws` CLI authenticated):

```
./machines/bench-gcp.sh c4-standard-8-lssd
./machines/bench-aws.sh c7i.12xlarge
```

We use `NAPKIN_MACHINE` and `NAPKIN_CONFIG` to label runs in the CSV. For example, `NAPKIN_MACHINE=aws-c7i.12xlarge` tells the CSV that a run used that particular C7i architecture. Machines can also be tuned at the kernel differently. If you don't set `NAPKIN_CONFIG`, it defaults to `baseline`, which means it uses the stock kernel. We benchmark on various machines and to reduce surprises, we set the config to `NAPKIN_CONFIG=bench_stable` when we tune the kernel for stable measurements.

For cloud VMs, see [machines/README.md](machines/README.md).
