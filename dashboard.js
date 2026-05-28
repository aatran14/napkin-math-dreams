var OPERATIONS = {
  seq_mem_read_single:   { label: "Sequential Memory R/W (64 bytes)", sub: "Single Thread", metric: "throughput" },
  seq_mem_read_threaded: { label: "Sequential Memory R/W (64 bytes)", sub: "Threaded", metric: "throughput" },
  mem_random_rw:         { label: "Random Memory R/W (64 bytes)", metric: "latency" },
  hash_non_crypto:       { label: "Hashing, non-crypto (64 bytes)", metric: "throughput" },
  hash_crypto:           { label: "Hashing, crypto-safe (64 bytes)", metric: "throughput" },
  hash_siphash:          { label: "Hashing, SipHash (64 bytes)", metric: "throughput" },
  syscall:               { label: "System Call", metric: "latency" },
  context_switch:        { label: "Context Switch", metric: "latency" },
  ssd_read_seq:          { label: "Sequential SSD Read (8 KiB)", metric: "throughput" },
  ssd_write_no_fsync:    { label: "Sequential SSD Write, -fsync (8 KiB)", metric: "throughput" },
  ssd_write_fsync:       { label: "Sequential SSD Write, +fsync (8 KiB)", metric: "throughput" },
  ssd_read_random:       { label: "Random SSD Read (8 KiB)", metric: "throughput" },
  tcp_echo:              { label: "TCP Echo Server (32 KiB)", metric: "throughput" },
  sort_64bit:            { label: "Sorting (64-bit integers)", metric: "throughput" },
  compression:           { label: "Compression (LZ4)", metric: "throughput" },
  decompression:         { label: "Decompression (LZ4)", metric: "throughput" },
  serialization_fast:    { label: "Fast Serialization (bincode)", metric: "throughput" },
  deserialization_fast:  { label: "Fast Deserialization (bincode)", metric: "throughput" },
  serialization:         { label: "Serialization (JSON)", metric: "throughput" },
  deserialization:       { label: "Deserialization (JSON)", metric: "throughput" }
};

var OP_ORDER = [
  "seq_mem_read_single", "seq_mem_read_threaded",
  "hash_non_crypto", "mem_random_rw",
  "hash_siphash",
  "syscall", "hash_crypto",
  "ssd_read_seq", "context_switch",
  "ssd_write_no_fsync", "tcp_echo",
  "ssd_read_random",
  "decompression", "compression",
  "sort_64bit",
  "serialization_fast", "deserialization_fast",
  "ssd_write_fsync",
  "serialization", "deserialization"
];

fetch("data/dead.csv", { cache: "no-store" })
  .then(function(r) { return r.text(); })
  .then(function(csvText) {
    var rows = parseCSV(csvText);

    var byMachine = {};
    rows.forEach(function(row) {
      var key = row.machine + " | " + row.cpu;
      if (!byMachine[key]) byMachine[key] = { machine: row.machine, cpu: row.cpu, date: row.date, ops: {} };
      if (!byMachine[key].ops[row.operation] || row.date > byMachine[key].date) {
        byMachine[key].ops[row.operation] = row;
      }
      if (row.date > byMachine[key].date) byMachine[key].date = row.date;
    });

    var container = document.getElementById("charts");

    Object.keys(byMachine).forEach(function(key) {
      var m = byMachine[key];

      var h2 = document.createElement("h2");
      h2.textContent = m.machine + (m.cpu ? " (" + m.cpu + ")" : "") + " — " + m.date;
      container.appendChild(h2);

      var table = document.createElement("table");
      var thead = document.createElement("thead");
      var hr = document.createElement("tr");
      ["Operation", "Latency", "Throughput", "1 MiB", "1 GiB"].forEach(function(h) {
        var th = document.createElement("th");
        th.textContent = h;
        hr.appendChild(th);
      });
      thead.appendChild(hr);
      table.appendChild(thead);

      var tbody = document.createElement("tbody");
      OP_ORDER.forEach(function(op) {
        var info = OPERATIONS[op];
        if (!info) return;
        var row = m.ops[op];
        if (!row) return;

        var lat_ns = parseFloat(row.latency_ns);
        var thr = parseFloat(row.throughput_bytes_s);

        var tr = document.createElement("tr");

        var label = info.sub ? "├ " + info.sub : info.label;
        addCell(tr, label);
        addCell(tr, isFinite(lat_ns) ? fmtLatency(lat_ns) : "");
        addCell(tr, isFinite(thr) ? fmtThroughput(thr) : "");
        addCell(tr, isFinite(thr) ? fmtTime(1048576 / thr) : "");
        addCell(tr, isFinite(thr) ? fmtTime(1073741824 / thr) : "");

        tbody.appendChild(tr);
      });
      table.appendChild(tbody);
      container.appendChild(table);
    });
  });

function addCell(tr, text) {
  var td = document.createElement("td");
  td.textContent = text;
  tr.appendChild(td);
}

function parseCSV(text) {
  var lines = text.split(/\r?\n/).filter(function(l) { return l.trim(); });
  var headers = lines[0].split(",");
  var rows = [];
  for (var i = 1; i < lines.length; i++) {
    var cells = lines[i].split(",");
    var row = {};
    for (var j = 0; j < headers.length; j++) {
      row[headers[j].trim()] = (cells[j] || "").trim();
    }
    rows.push(row);
  }
  return rows;
}

function fmtLatency(ns) {
  if (ns >= 1e9) return (ns / 1e9).toFixed(1) + " s";
  if (ns >= 1e6) return (ns / 1e6).toFixed(1) + " ms";
  if (ns >= 1e3) return (ns / 1e3).toFixed(1) + " μs";
  return ns.toFixed(1) + " ns";
}

function fmtThroughput(v) {
  if (v >= 1073741824) return (v / 1073741824).toFixed(0) + " GiB/s";
  if (v >= 1048576) return (v / 1048576).toFixed(0) + " MiB/s";
  return (v / 1024).toFixed(0) + " KiB/s";
}

function fmtTime(secs) {
  if (secs >= 60) return (secs / 60).toFixed(0) + "m";
  if (secs >= 1) return secs.toFixed(0) + "s";
  if (secs >= 0.001) return (secs * 1000).toFixed(0) + " ms";
  if (secs >= 0.000001) return (secs * 1000000).toFixed(0) + " μs";
  return (secs * 1000000000).toFixed(0) + " ns";
}
