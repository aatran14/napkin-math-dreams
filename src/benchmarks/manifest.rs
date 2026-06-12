//! Benchmark manifest: one `.toml` per row under `benchmarks/`.
//!
//! Add a row:
//!   1. `pub fn foo() -> Measurement` in `src/benchmarks/<section>.rs`
//!   2. Register in `registry()` below
//!   3. Drop `benchmarks/<section>/foo.toml` with `nightly = true|false`
//!
//! Dev:  `cargo run --release --bin daily -- run memory/random_read`
//! Fleet: `cargo run --release --bin daily` (all rows with `nightly = true`)

use crate::benchmarks::{
    compression, hash, memory, network, serialization, sort, ssd, syscall, Measurement,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const SECTION_ORDER: &[&str] = &[
    "memory",
    "syscall",
    "hash",
    "cpu",
    "serialization",
    "ssd",
    "network",
];

#[derive(Debug, Clone)]
pub struct Row {
    pub id: String,
    pub key: String,
    pub section: String,
    pub nightly: bool,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Section {
    pub name: String,
    pub rows: Vec<Row>,
}

#[derive(Deserialize)]
struct RowDef {
    key: String,
    section: String,
    nightly: bool,
}

pub fn registry() -> HashMap<&'static str, fn() -> Measurement> {
    let mut m: HashMap<&'static str, fn() -> Measurement> = HashMap::new();

    macro_rules! insert {
        ($id:expr, $f:expr) => {
            if m.insert($id, $f).is_some() {
                panic!("duplicate manifest id: {}", $id);
            }
        };
    }

    insert!("memory/seq_read_single", memory::seq_read_single);
    insert!("memory/seq_read_threaded", memory::seq_read_threaded);
    insert!("memory/seq_write_single", memory::seq_write_single);
    insert!("memory/seq_write_threaded", memory::seq_write_threaded);
    insert!("memory/random_read", memory::random_read);
    insert!("memory/random_write", memory::random_write);

    insert!("syscall/getpid", syscall::getpid);
    insert!("syscall/context_switch", syscall::context_switch);

    insert!("hash/non_crypto", hash::non_crypto);
    insert!("hash/crypto", hash::crypto);

    insert!("cpu/sort_u64", sort::sort_u64);
    insert!("cpu/compress", compression::compress);
    insert!("cpu/decompress", compression::decompress);

    insert!("serialization/fast_serialize", serialization::fast_serialize);
    insert!(
        "serialization/fast_deserialize",
        serialization::fast_deserialize
    );
    insert!("serialization/slow_serialize", serialization::slow_serialize);
    insert!(
        "serialization/slow_deserialize",
        serialization::slow_deserialize
    );

    insert!("ssd/seq_read", ssd::seq_read);
    insert!("ssd/seq_write_no_fsync", ssd::seq_write_no_fsync);
    insert!("ssd/seq_write_fsync", ssd::seq_write_fsync);
    insert!("ssd/random_read", ssd::random_read);

    insert!("network/tcp_echo", network::tcp_echo);

    m
}

pub fn benchmarks_root() -> PathBuf {
    if let Ok(root) = std::env::var("NAPKIN_BENCHMARKS") {
        return PathBuf::from(root);
    }
    PathBuf::from("benchmarks")
}

pub fn load_rows() -> Vec<Row> {
    let root = benchmarks_root();
    let mut rows = Vec::new();
    walk_rows(&root, &root, &mut rows);
    rows.sort_by(|a, b| {
        section_rank(&a.section)
            .cmp(&section_rank(&b.section))
            .then_with(|| a.id.cmp(&b.id))
    });
    rows
}

fn section_rank(section: &str) -> usize {
    SECTION_ORDER
        .iter()
        .position(|s| *s == section)
        .unwrap_or(SECTION_ORDER.len())
}

fn walk_rows(root: &Path, dir: &Path, out: &mut Vec<Row>) {
    let entries = fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {}", dir.display(), e));
    for entry in entries {
        let entry = entry.unwrap_or_else(|e| panic!("read dir entry in {}: {}", dir.display(), e));
        let path = entry.path();
        if path.is_dir() {
            walk_rows(root, &path, out);
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("toml") {
            continue;
        }
        if path.file_name().and_then(|s| s.to_str()) == Some("_template.toml") {
            continue;
        }
        out.push(parse_row(root, &path));
    }
}

fn parse_row(root: &Path, path: &Path) -> Row {
    let rel = path
        .strip_prefix(root)
        .unwrap_or_else(|_| panic!("{} not under {}", path.display(), root.display()));
    let id = rel
        .with_extension("")
        .to_string_lossy()
        .replace('\\', "/");

    let contents = fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read {}: {}", path.display(), e));
    let def: RowDef = toml::from_str(&contents)
        .unwrap_or_else(|e| panic!("parse {}: {}", path.display(), e));

    let expected_section = rel
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("");
    if def.section != expected_section {
        panic!(
            "{}: section = {:?} but file lives under {:?}",
            path.display(),
            def.section,
            expected_section
        );
    }

    let reg = registry();
    if !reg.contains_key(id.as_str()) {
        panic!(
            "{}: no registry entry for id {:?} — add it to manifest.rs registry()",
            path.display(),
            id
        );
    }

    Row {
        id,
        key: def.key,
        section: def.section,
        nightly: def.nightly,
        path: path.to_path_buf(),
    }
}

pub fn list_rows(nightly_only: bool, section: Option<&str>) -> Vec<Row> {
    load_rows()
        .into_iter()
        .filter(|r| !nightly_only || r.nightly)
        .filter(|r| section.map(|s| r.section == s).unwrap_or(true))
        .collect()
}

pub fn run_ids(ids: &[String]) -> Vec<Measurement> {
    let reg = registry();
    ids.iter()
        .map(|id| {
            let f = *reg
                .get(id.as_str())
                .unwrap_or_else(|| panic!("unknown benchmark id: {}", id));
            f()
        })
        .collect()
}

pub fn run_rows(rows: &[Row]) -> Vec<Measurement> {
    run_ids(&rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>())
}

pub fn run_nightly() -> Vec<Measurement> {
    run_rows(&list_rows(true, None))
}

pub fn sections(nightly_only: bool) -> Vec<Section> {
    let mut out: Vec<Section> = Vec::new();
    for row in list_rows(nightly_only, None) {
        if let Some(section) = out.last_mut() {
            if section.name == row.section {
                section.rows.push(row);
                continue;
            }
        }
        out.push(Section {
            name: row.section.clone(),
            rows: vec![row],
        });
    }
    out
}

pub fn resolve_ids(requests: &[String]) -> Vec<String> {
    let rows = load_rows();
    let mut ids = Vec::new();
    for req in requests {
        if req.contains('/') {
            if rows.iter().any(|r| r.id == *req) {
                ids.push(req.clone());
            } else {
                panic!("unknown benchmark id: {}", req);
            }
            continue;
        }
        let matches: Vec<_> = rows.iter().filter(|r| r.key == *req).collect();
        match matches.len() {
            0 => panic!("unknown benchmark key: {}", req),
            1 => ids.push(matches[0].id.clone()),
            _ => panic!(
                "ambiguous key {:?} — use a full id like memory/{}",
                req, req
            ),
        }
    }
    ids
}
