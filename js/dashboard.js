// Napkin-math table (SIMON.md order). Trend column:
//   spark       — throughput (or latency) over daily runs
//   dual spark  — memoryPerLine rows: Latency + GiB/s side by side
//   trendMerge  — section-level merged latency + throughput charts (seq memory)
//   percentile  — p50 vs p99 spread across runs (tail ops; object-storage style)
//   none        — numbers only

var MEMORY_TREND_READ = "#e85d04";
var MEMORY_TREND_WRITE = "#0891b2";
var MEMORY_THROUGHPUT_Y_MIN = 0;
var MEMORY_THROUGHPUT_Y_MAX = 200;

var TABLE = [
  {
    title: "Sequential Memory R/W (64 bytes)",
    napkinLatency: "0.5 ns",
    trendMerge: true,
    rows: [
      { key: "seq_mem_read_single", sub: "Single Thread (read)", metric: "throughput", trend: "spark", color: "#e85d04", memoryPerLine: true },
      { key: "seq_mem_read_threaded", sub: "Threaded (read)", metric: "throughput", trend: "spark", color: "#111", memoryPerLine: true },
      { key: "seq_mem_write_single", sub: "Single Thread (write)", metric: "throughput", trend: "spark", color: "#0891b2", memoryPerLine: true },
      { key: "seq_mem_write_threaded", sub: "Threaded (write)", metric: "throughput", trend: "spark", color: "#6b7280", memoryPerLine: true }
    ]
  },
  {
    title: "Random Memory R/W (64 bytes)",
    napkinLatency: "20 ns",
    rows: [
      { key: "mem_random_read", sub: "Read", metric: "latency", trend: "percentile" },
      { key: "mem_random_write", sub: "Write", metric: "latency", trend: "percentile" }
    ]
  },
  {
    title: "Hashing, not crypto-safe (64 bytes)",
    napkinLatency: "10 ns",
    rows: [{ key: "hash_non_crypto", metric: "throughput", trend: "spark", color: "#e85d04" }]
  },
  {
    title: "Hashing, crypto-safe (64 bytes)",
    napkinLatency: "100 ns",
    rows: [{ key: "hash_crypto", metric: "throughput", trend: "none" }]
  },
  {
    title: "Hashing, SipHash (64 bytes)",
    napkinLatency: "",
    rows: [{ key: "hash_siphash", metric: "throughput", trend: "none" }]
  },
  {
    title: "Fast Serialization (bincode)",
    napkinLatency: "N/A",
    rows: [{ key: "serialization_fast", metric: "throughput", trend: "spark", color: "#0891b2" }]
  },
  {
    title: "Fast Deserialization (bincode)",
    napkinLatency: "N/A",
    rows: [{ key: "deserialization_fast", metric: "throughput", trend: "spark", color: "#0891b2" }]
  },
  {
    title: "System Call",
    napkinLatency: "300 ns",
    rows: [{ key: "syscall", metric: "latency", trend: "percentile" }]
  },
  {
    title: "Sequential SSD read (8 KiB)",
    napkinLatency: "1 μs",
    rows: [{ key: "ssd_read_seq", metric: "throughput", trend: "spark", color: "#6b7280" }]
  },
  {
    title: "Context Switch",
    napkinLatency: "10 μs",
    rows: [{ key: "context_switch", metric: "latency", trend: "percentile" }]
  },
  {
    title: "Sequential SSD write, -fsync (8 KiB)",
    napkinLatency: "2 μs",
    rows: [{ key: "ssd_write_no_fsync", metric: "throughput", trend: "spark", color: "#6b7280" }]
  },
  {
    title: "TCP Echo Server (32 KiB)",
    napkinLatency: "50 μs",
    rows: [{ key: "tcp_echo", metric: "throughput", trend: "spark", color: "#6b7280" }]
  },
  {
    title: "Random SSD Read (8 KiB)",
    napkinLatency: "100 μs",
    rows: [{ key: "ssd_read_random", metric: "latency", trend: "percentile" }]
  },
  {
    title: "Decompression (LZ4)",
    napkinLatency: "N/A",
    rows: [{ key: "decompression", metric: "throughput", trend: "none" }]
  },
  {
    title: "Compression (LZ4)",
    napkinLatency: "N/A",
    rows: [{ key: "compression", metric: "throughput", trend: "none" }]
  },
  {
    title: "Sorting (64-bit integers)",
    napkinLatency: "N/A",
    rows: [{ key: "sort_64bit", metric: "throughput", trend: "none" }]
  },
  {
    title: "Sequential SSD write, +fsync (8 KiB)",
    napkinLatency: "300 μs",
    rows: [{ key: "ssd_write_fsync", metric: "latency", trend: "percentile" }]
  },
  {
    title: "Serialization (JSON)",
    napkinLatency: "N/A",
    rows: [{ key: "serialization", metric: "throughput", trend: "none" }]
  },
  {
    title: "Deserialization (JSON)",
    napkinLatency: "N/A",
    rows: [{ key: "deserialization", metric: "throughput", trend: "none" }]
  }
];

var GI_BYTES = 1073741824;
var CACHE_LINES_PER_GI = GI_BYTES / 64;
var STROKE_LATENCY = "#06b6d4";
var STROKE_THROUGHPUT = "#1e293b";
var popover = null;
var popover2 = null;
var popoverMode = "spark";
var POPOVER_CHART_W = 444;
var POPOVER_CHART_H = 250;
var POPOVER_GAP = 28;
var SPARK_TIMELINE_DAYS = 14;

var CHANGELOG = [];

var LEGACY_OP_KEYS = { mem_random_read: "mem_random_rw" };

function resolveOpKey(m, rowKey) {
  if (m.ops[rowKey]) return rowKey;
  var legacy = LEGACY_OP_KEYS[rowKey];
  if (legacy && m.ops[legacy]) return legacy;
  return null;
}

function hasOp(m, rowKey) {
  return !!resolveOpKey(m, rowKey);
}

function getOp(m, rowKey) {
  var k = resolveOpKey(m, rowKey);
  return k ? m.ops[k] : null;
}

function getSeries(m, rowKey) {
  var k = resolveOpKey(m, rowKey);
  return k ? m.series[k] : null;
}

function boot(rows, changelog) {
    CHANGELOG = changelog || [];
    popover = document.getElementById("spark-popover");
    popover2 = document.getElementById("spark-popover-2");

    var byMachine = {};
    rows.forEach(function(row) {
      if (!row.machine || row.machine === "machine") return;
      var key = row.machine;
      if (!byMachine[key]) {
        byMachine[key] = { machine: key, date: "", ops: {}, series: {} };
      }
      var m = byMachine[key];
      var day = dayKey(row.date);

      if (!m.series[row.operation]) m.series[row.operation] = {};
      var slot = m.series[row.operation][day];
      if (!slot || row.date >= slot.date) {
        m.series[row.operation][day] = row;
      }

      var prev = m.ops[row.operation];
      if (!prev || row.date > prev.date) {
        m.ops[row.operation] = row;
      }
      if (row.date > m.date) m.date = row.date;
    });

    var machines = Object.keys(byMachine).sort(function(a, b) {
      return byMachine[b].date.localeCompare(byMachine[a].date);
    });

    var bar = document.querySelector(".machine-bar");
    var table = document.getElementById("bench-table");
    var errEl = document.getElementById("bench-error");
    var toc = document.getElementById("bench-toc");

    if (!machines.length) {
      bar.hidden = true;
      table.hidden = true;
      if (toc) toc.hidden = true;
      var stack = document.querySelector(".page-stack");
      if (stack) stack.classList.remove("has-toc");
      if (errEl) {
        errEl.hidden = false;
        errEl.textContent = "No benchmark data yet.";
      }
      return;
    }
    if (errEl) errEl.hidden = true;

    var initial = machineFromHash(machines) || machines[0];
    var pickerState = { selected: initial };

    initMachinePicker(machines, pickerState, function(key) {
      hidePopover();
      history.replaceState(null, "", "#m=" + encodeURIComponent(key));
      renderMachine(key);
    });

    function renderMachine(key) {
      pickerState.selected = key;
      setPickerLabel(key);
      updatePickerActiveFromState(pickerState);
      var m = byMachine[key];
      while (table.tBodies.length) table.removeChild(table.tBodies[0]);
      renderToc(m, toc);
      var anyRow = false;
      var sectionIdx = 0;
      var prevGroup = null;

      TABLE.forEach(function(sec) {
        var present = sec.rows.filter(function(row) { return hasOp(m, row.key); });
        if (!present.length) return;

        var group = tocGroup(sec.title);
        if (prevGroup !== null && group !== prevGroup) sectionIdx++;
        prevGroup = group;

        var secBody = document.createElement("tbody");
        secBody.id = sectionSlug(sec.title);
        if (sectionIdx % 2 === 1) secBody.className = "section-alt";

        var showHead = sec.napkinLatency || present.some(function(r) { return r.sub; });
        if (showHead) {
          var head = document.createElement("tr");
          head.className = "section-head";
          addCell(head, sec.title);
          addCell(head, sec.napkinLatency || "");
          addCell(head, "");
          addCell(head, "");
          addCell(head, "");
          addCell(head, "");
          secBody.appendChild(head);
        }

        present.forEach(function(row, rowIdx) {
          var csvRow = getOp(m, row.key);
          var opLabel = row.sub ? ("├ " + row.sub) : sec.title;
          var tr = document.createElement("tr");
          addCell(tr, opLabel);
          fillMetricCells(tr, row, csvRow);
          if (sec.trendMerge) {
            if (rowIdx === 0) {
              var trendTd = document.createElement("td");
              trendTd.className = "trend-cell";
              trendTd.rowSpan = present.length;
              appendMergedMemoryTrend(trendTd, present, m, sec.title, key);
              tr.appendChild(trendTd);
            }
          } else {
            addTrendCell(tr, row, m, opLabel, key);
          }
          secBody.appendChild(tr);
          anyRow = true;
        });

        table.appendChild(secBody);
      });

      if (!anyRow) {
        var secBody = document.createElement("tbody");
        var tr = document.createElement("tr");
        var td = document.createElement("td");
        td.colSpan = 6;
        td.textContent = "No rows for this machine.";
        tr.appendChild(td);
        secBody.appendChild(tr);
        table.appendChild(secBody);
      }
      requestAnimationFrame(function() {
        updateStickyChrome();
        syncTocActive();
      });
    }

    window.addEventListener("resize", updateStickyChrome);
    renderMachine(initial);
    renderChangelog();
}

function fail(err) {
    var errEl = document.getElementById("bench-error");
    var bar = document.querySelector(".machine-bar");
    var table = document.getElementById("bench-table");
    var toc = document.getElementById("bench-toc");
    if (bar) bar.hidden = true;
    if (table) table.hidden = true;
    if (toc) toc.hidden = true;
    var stack = document.querySelector(".page-stack");
    if (stack) stack.classList.remove("has-toc");
    if (errEl) {
      errEl.hidden = false;
      errEl.textContent = "Could not load benchmark data — " + err.message
        + ". Run `make publish`, or serve the repo (python3 -m http.server) instead of opening the file directly.";
    }
    console.error(err);
}

function renderChangelog() {
    var el = document.getElementById("changelog");
    if (!el) return;
    if (!CHANGELOG.length) { el.hidden = true; return; }
    el.hidden = false;
    el.innerHTML = "<h2>Changelog</h2>";
    var ul = document.createElement("ul");
    ul.className = "changelog-list";
    CHANGELOG.slice().sort(function(a, b) {
      return (b.date || "").localeCompare(a.date || "");
    }).forEach(function(entry) {
      var li = document.createElement("li");
      var tag = entry.type ? ("[" + entry.type + "] ") : "";
      li.textContent = (entry.date || "") + " — " + tag + (entry.description || "");
      ul.appendChild(li);
    });
    el.appendChild(ul);
}

function fillMetricCells(tr, row, csvRow) {
  var lat_ns = parseFloat(csvRow.latency_ns);
  var thr = parseFloat(csvRow.throughput_bytes_s);

  var latText = "";
  if (isFinite(lat_ns) && !row.memoryPerLine) {
    latText = fmtLatency(lat_ns);
  }
  addCell(tr, latText);
  addCell(tr, isFinite(thr) ? fmtThroughput(thr) : "");
  addCell(tr, isFinite(thr) ? fmtTime(1048576 / thr) : "");
  addCell(tr, isFinite(thr) ? fmtTime(GI_BYTES / thr) : "");
}

function addTrendCell(tr, row, m, label, machineKey) {
  var td = document.createElement("td");
  td.className = "trend-cell";

  if (!row.trend || row.trend === "none") {
    td.textContent = "";
    tr.appendChild(td);
    return;
  }

  if (row.trend === "spark") {
    if (row.memoryPerLine) {
      appendDualSparkTrend(td, row, m, label, machineKey);
    } else {
      appendSingleSparkTrend(td, row, m, label, machineKey);
    }
    tr.appendChild(td);
    return;
  }

  if (row.trend === "percentile") {
    var pct = percentileFromSeries(getSeries(m, row.key));
    if (!pct) {
      td.textContent = "—";
      tr.appendChild(td);
      return;
    }
    var wrap = document.createElement("div");
    wrap.className = "pct-wrap";
    wrap.appendChild(makePercentileChart(pct));
    wrap.addEventListener("mouseenter", function(e) {
      showPercentilePopover(e, pct, label, machineKey);
    });
    wrap.addEventListener("mousemove", positionPopover);
    wrap.addEventListener("mouseleave", hidePopover);
    td.appendChild(wrap);
    tr.appendChild(td);
  }
}

function memoryRowThreaded(row) {
  return row.key.indexOf("_threaded") !== -1;
}

function memoryRowWrite(row) {
  return row.key.indexOf("_write_") !== -1;
}

function memoryTrendSeries(rows, m, metric) {
  return rows.map(function(row) {
    var write = memoryRowWrite(row);
    var threaded = memoryRowThreaded(row);
    return {
      points: seriesPoints(getSeries(m, row.key), metric, row),
      color: write ? MEMORY_TREND_WRITE : MEMORY_TREND_READ,
      dash: threaded ? "4,2" : null,
      label: row.sub || row.key
    };
  }).filter(function(s) { return s.points.length; });
}

function appendMergedMemoryTrend(td, rows, m, sectionTitle, machineKey) {
  var latSeries = memoryTrendSeries(rows, m, "latency");
  var thrSeries = memoryTrendSeries(rows, m, "throughput");
  if (!latSeries.length && !thrSeries.length) {
    td.textContent = "—";
    return;
  }

  var wrap = document.createElement("div");
  wrap.className = "spark-merged";

  var legendSeries = latSeries.length ? latSeries : thrSeries;
  wrap.appendChild(makeMemoryTrendLegendDetailed(legendSeries));

  if (latSeries.length) {
    wrap.appendChild(makeMergedSparkBlock(alignMultiSeries(latSeries), "Latency", "ns"));
  }
  if (thrSeries.length) {
    wrap.appendChild(makeMergedSparkBlock(alignMultiSeries(thrSeries), "Throughput", "GiB/s"));
  }

  wrap.addEventListener("mouseenter", function(e) {
    showMergedMemoryPopover(e, latSeries, thrSeries, sectionTitle, machineKey);
  });
  wrap.addEventListener("mousemove", positionPopover);
  wrap.addEventListener("mouseleave", hidePopover);
  td.appendChild(wrap);
}

function memorySparkOpts(yUnit) {
  var opts = { yUnit: yUnit };
  if (yUnit === "GiB/s") {
    opts.yScaleLock = true;
    opts.yMin = MEMORY_THROUGHPUT_Y_MIN;
    opts.yMax = MEMORY_THROUGHPUT_Y_MAX;
    opts.yTicks = [0, 100, 200];
  }
  return opts;
}

function makeMergedSparkBlock(alignedSeries, unitLabel, yUnit) {
  var block = document.createElement("div");
  block.className = "spark-merged-block";
  var lbl = document.createElement("div");
  lbl.className = "spark-dual-label";
  lbl.textContent = unitLabel;
  block.appendChild(lbl);
  var svg = makeMultiSparkSvg(148, 44, alignedSeries, memorySparkOpts(yUnit));
  svg.classList.add("spark-mini");
  svg.setAttribute("width", "100%");
  svg.setAttribute("height", "44");
  block.appendChild(svg);
  return block;
}

function makeMemoryTrendLegendDetailed(seriesList) {
  var leg = makeSparkLegend(seriesList.map(function(s) {
    return { color: s.color, dash: s.dash, label: s.label };
  }));
  leg.classList.add("spark-merged-legend");
  return leg;
}

function makeSparkLegend(items) {
  var leg = document.createElement("div");
  leg.className = "spark-legend";
  items.forEach(function(item) {
    var row = document.createElement("div");
    row.className = "spark-legend-item";
    var swatch = document.createElement("span");
    swatch.className = "spark-legend-swatch" + (item.dash ? " dashed" : "");
    if (item.dash) {
      swatch.style.borderColor = item.color;
    } else {
      swatch.style.background = item.color;
    }
    row.appendChild(swatch);
    var text = document.createElement("span");
    text.textContent = item.label;
    row.appendChild(text);
    leg.appendChild(row);
  });
  return leg;
}

function makeMergedPopoverChart(alignedSeries, rawSeries, svgOpts) {
  var root = document.createElement("div");
  root.className = "spark-popover-merged";
  var opts = Object.assign({}, svgOpts);
  var w = opts.w;
  var h = opts.h;
  delete opts.w;
  delete opts.h;
  root.appendChild(makeMultiSparkSvg(w, h, alignedSeries, opts));
  root.appendChild(makeMemoryTrendLegendDetailed(rawSeries));
  return root;
}

function alignMultiSeries(seriesList) {
  var daySet = {};
  seriesList.forEach(function(s) {
    s.points.forEach(function(p) { daySet[p.day] = true; });
  });
  var days = Object.keys(daySet).sort();
  return seriesList.map(function(s) {
    var byDay = {};
    s.points.forEach(function(p) { byDay[p.day] = p.v; });
    return {
      color: s.color,
      dash: s.dash,
      points: days.map(function(day) {
        return { day: day, v: byDay[day] != null ? byDay[day] : null };
      })
    };
  });
}

function showMergedMemoryPopover(e, latSeries, thrSeries, sectionTitle, machineKey) {
  popoverMode = "dual";
  var machine = shortMachine(machineKey);
  var latAligned = latSeries.length
    ? alignMultiSeries(latSeries.map(function(s) {
        return { color: s.color, dash: s.dash, points: expandPointsTimeline(s.points, SPARK_TIMELINE_DAYS) };
      }))
    : [];
  var thrAligned = thrSeries.length
    ? alignMultiSeries(thrSeries.map(function(s) {
        return { color: s.color, dash: s.dash, points: expandPointsTimeline(s.points, SPARK_TIMELINE_DAYS) };
      }))
    : [];
  fillPopover(popover, "spark-popover-title", "spark-popover-body", machine + " · Latency",
    latAligned.length
      ? makeMergedPopoverChart(latAligned, latSeries, {
          w: POPOVER_CHART_W,
          h: POPOVER_CHART_H,
          showDots: true,
          labeled: true,
          yUnit: "ns",
          xLabel: "Date",
          timelineDays: SPARK_TIMELINE_DAYS
        })
      : null);
  fillPopover(popover2, "spark-popover-title-2", "spark-popover-body-2", machine + " · Throughput",
    thrAligned.length
      ? makeMergedPopoverChart(thrAligned, thrSeries, Object.assign({
          w: POPOVER_CHART_W,
          h: POPOVER_CHART_H,
          showDots: true,
          labeled: true,
          yUnit: "GiB/s",
          xLabel: "Date",
          timelineDays: SPARK_TIMELINE_DAYS
        }, memorySparkOpts("GiB/s")))
      : null);
  popover.hidden = !latAligned.length;
  popover2.hidden = !thrAligned.length;
  positionDualPopover(e);
  requestAnimationFrame(function() {
    if (!popover.hidden) popover.classList.add("show");
    if (!popover2.hidden) popover2.classList.add("show");
  });
}

function appendSingleSparkTrend(td, row, m, label, machineKey) {
  var points = seriesPoints(getSeries(m, row.key), row.metric, row);
  if (!points.length) {
    td.textContent = "—";
    return;
  }
  var unit = sparkYUnit(row.metric, row);
  var wrap = document.createElement("div");
  wrap.className = "spark-wrap";
  wrap.appendChild(makeSparkMiniVisual(points, row.color || "#e85d04", unit));
  wrap.addEventListener("mouseenter", function(e) {
    showSparkPopover(e, points, label, row.color || "#e85d04", machineKey, row.metric, row);
  });
  wrap.addEventListener("mousemove", positionPopover);
  wrap.addEventListener("mouseleave", hidePopover);
  td.appendChild(wrap);
}

function appendDualSparkTrend(td, row, m, label, machineKey) {
  var latPoints = seriesPoints(getSeries(m, row.key), "latency", row);
  var thrPoints = seriesPoints(getSeries(m, row.key), "throughput", row);
  if (!latPoints.length && !thrPoints.length) {
    td.textContent = "—";
    return;
  }
  var wrap = document.createElement("div");
  wrap.className = "spark-dual";
  if (latPoints.length) {
    wrap.appendChild(makeSparkMiniVisual(latPoints, STROKE_LATENCY, "Latency"));
  }
  if (thrPoints.length) {
    wrap.appendChild(makeSparkMiniVisual(thrPoints, STROKE_THROUGHPUT, "GiB/s"));
  }
  wrap.addEventListener("mouseenter", function(e) {
    showDualSparkPopover(e, latPoints, thrPoints, label, machineKey, row);
  });
  wrap.addEventListener("mousemove", positionPopover);
  wrap.addEventListener("mouseleave", hidePopover);
  td.appendChild(wrap);
}

function makeSparkMiniVisual(points, color, unitLabel) {
  var item = document.createElement("div");
  item.className = "spark-dual-item";
  var svg = makeSparkSvg(96, 38, points, color);
  svg.classList.add("spark-mini");
  svg.setAttribute("width", "100%");
  svg.setAttribute("height", "38");
  item.appendChild(svg);
  var lbl = document.createElement("div");
  lbl.className = "spark-dual-label";
  lbl.textContent = unitLabel;
  item.appendChild(lbl);
  return item;
}

function sparkYUnit(metric, row) {
  if (metric === "throughput") return "GiB/s";
  if (row && row.memoryPerLine) return "Latency";
  return "μs";
}

function seriesPoints(byDay, metric, row) {
  if (!byDay) return [];
  return Object.keys(byDay).sort().map(function(day) {
    var csvRow = byDay[day];
    var v;
    if (metric === "throughput") {
      v = parseFloat(csvRow.throughput_bytes_s) / 1073741824;
      if (!isFinite(v)) return null;
    } else {
      v = parseFloat(csvRow.latency_ns);
      if (!isFinite(v)) return null;
      if (row && row.memoryPerLine) {
        v = perCacheLineNs(v);
      } else {
        v = v / 1000;
      }
    }
    return { day: day, v: v };
  }).filter(Boolean);
}

function percentileFromSeries(byDay) {
  if (!byDay) return null;
  var vals = Object.keys(byDay).map(function(day) {
    return parseFloat(byDay[day].latency_ns);
  }).filter(isFinite).sort(function(a, b) { return a - b; });
  if (!vals.length) return null;
  var p50i = Math.floor(vals.length * 0.5);
  var p99i = Math.min(vals.length - 1, Math.floor(vals.length * 0.99));
  return {
    p50: vals[p50i],
    p99: vals[p99i],
    n: vals.length,
    min: vals[0],
    max: vals[vals.length - 1]
  };
}

function makePercentileChart(pct, opts) {
  opts = opts || {};
  var mini = !opts.labeled;
  var root = document.createElement("div");
  root.className = mini ? "pct-chart pct-chart-mini" : "pct-chart pct-chart-large";

  var maxV = Math.max(pct.p99, pct.p50, 1);
  [
    { label: "p50", val: pct.p50 },
    { label: "p99", val: pct.p99 }
  ].forEach(function(row) {
    var rowEl = document.createElement("div");
    rowEl.className = "pct-row";

    var lbl = document.createElement("span");
    lbl.className = "pct-label";
    lbl.textContent = row.label;

    var track = document.createElement("span");
    track.className = "pct-bar-track";
    var bar = document.createElement("span");
    bar.className = "pct-bar";
    bar.style.width = Math.max((row.val / maxV) * 100, 4) + "%";
    track.appendChild(bar);

    var val = document.createElement("span");
    val.className = "pct-val";
    val.textContent = fmtLatency(row.val);

    rowEl.appendChild(lbl);
    rowEl.appendChild(track);
    rowEl.appendChild(val);
    root.appendChild(rowEl);
  });

  if (opts.labeled) {
    var footer = document.createElement("div");
    footer.className = "pct-footer";
    footer.textContent = "Latency (across " + pct.n + " runs)";
    root.appendChild(footer);
  }

  return root;
}

function showSparkPopover(e, points, label, color, machineKey, metric, row) {
  popoverMode = "spark";
  var title = shortMachine(machineKey) + " — " + label;
  fillPopover(popover, "spark-popover-title", "spark-popover-body", title,
    makePopoverChart(points, color, row, metric));
  popover2.hidden = true;
  popover2.classList.remove("show");
  popover.hidden = false;
  positionPopover(e);
  requestAnimationFrame(function() { popover.classList.add("show"); });
}

function showDualSparkPopover(e, latPoints, thrPoints, label, machineKey, row) {
  popoverMode = "dual";
  var base = shortMachine(machineKey) + " — " + label;
  fillPopover(popover, "spark-popover-title", "spark-popover-body", base + " · latency (ns)",
    latPoints.length ? makePopoverChart(latPoints, STROKE_LATENCY, row, "latency", "latency (ns)") : null);
  fillPopover(popover2, "spark-popover-title-2", "spark-popover-body-2", base + " · throughput (GiB)",
    thrPoints.length ? makePopoverChart(thrPoints, STROKE_THROUGHPUT, row, "throughput", "throughput (GiB)") : null);
  popover.hidden = !latPoints.length;
  popover2.hidden = !thrPoints.length;
  positionDualPopover(e);
  requestAnimationFrame(function() {
    if (!popover.hidden) popover.classList.add("show");
    if (!popover2.hidden) popover2.classList.add("show");
  });
}

function fillPopover(el, titleId, bodyId, title, chart) {
  document.getElementById(titleId).textContent = title;
  var body = document.getElementById(bodyId);
  body.innerHTML = "";
  if (chart) body.appendChild(chart);
}

function makePopoverChart(points, color, row, metric, yUnitOverride) {
  var timeline = expandPointsTimeline(points, SPARK_TIMELINE_DAYS);
  return makeSparkSvg(420, 200, timeline, color, {
    showDots: true,
    labeled: true,
    yUnit: yUnitOverride || sparkYUnit(metric, row),
    xLabel: "Date",
    timelineDays: SPARK_TIMELINE_DAYS
  });
}

function showPercentilePopover(e, pct, label, machineKey) {
  popoverMode = "percentile";
  fillPopover(popover, "spark-popover-title", "spark-popover-body",
    shortMachine(machineKey) + " — " + label,
    makePercentileChart(pct, { labeled: true }));
  popover2.hidden = true;
  popover2.classList.remove("show");
  popover.hidden = false;
  positionPopover(e);
  requestAnimationFrame(function() { popover.classList.add("show"); });
}

function hidePopover() {
  popover.classList.remove("show");
  popover.hidden = true;
  popover2.classList.remove("show");
  popover2.hidden = true;
}

function positionPopover(e) {
  if (popoverMode === "dual") {
    positionDualPopover(e);
    return;
  }
  var pad = 12;
  var w = popoverMode === "percentile" ? 360 : POPOVER_CHART_W;
  var h = popoverMode === "percentile" ? 180 : POPOVER_CHART_H;
  placePopover(popover, e.clientX + pad, e.clientY + pad, w, h);
}

function positionDualPopover(e) {
  var pad = 12;
  var popW = POPOVER_CHART_W;
  var popH = POPOVER_CHART_H;
  var totalW = popW * 2 + POPOVER_GAP;
  var x = e.clientX - totalW / 2;
  var y = e.clientY + pad;

  if (x < pad) x = pad;
  if (x + totalW > window.innerWidth - pad) x = window.innerWidth - totalW - pad;
  if (y + popH > window.innerHeight - pad) y = e.clientY - popH - pad;
  if (y < pad) y = pad;

  if (!popover.hidden) {
    popover.style.left = x + "px";
    popover.style.top = y + "px";
  }
  if (!popover2.hidden) {
    popover2.style.left = (x + popW + POPOVER_GAP) + "px";
    popover2.style.top = y + "px";
  }
}

function placePopover(el, x, y, w, h) {
  var pad = 12;
  if (x + w > window.innerWidth) x = Math.max(pad, window.innerWidth - w - pad);
  if (y + h > window.innerHeight) y = Math.max(pad, window.innerHeight - h - pad);
  el.style.left = x + "px";
  el.style.top = y + "px";
}

function svgText(svg, x, y, text, opts) {
  opts = opts || {};
  var t = document.createElementNS("http://www.w3.org/2000/svg", "text");
  t.setAttribute("x", x);
  t.setAttribute("y", y);
  t.setAttribute("font-size", opts.size || "10");
  t.setAttribute("fill", opts.fill || "#444");
  t.setAttribute("font-family", "monospace");
  if (opts.anchor === "middle") t.setAttribute("text-anchor", "middle");
  if (opts.anchor === "end") t.setAttribute("text-anchor", "end");
  t.textContent = text;
  if (opts.rotate != null) {
    t.setAttribute("transform", "rotate(" + opts.rotate + " " + x + " " + y + ")");
  }
  svg.appendChild(t);
  return t;
}

function fmtSparkY(v, yUnit, axisSpan) {
  axisSpan = axisSpan || Math.abs(v) || 1;
  if (yUnit === "Latency" || yUnit === "latency (ns)") {
    if (axisSpan < 0.2) return v.toFixed(2);
    if (axisSpan < 2) return v.toFixed(1);
    return v.toFixed(0);
  }
  if (yUnit === "GiB/s" || yUnit === "throughput (GiB)") {
    if (axisSpan < 0.05) return v.toFixed(3);
    if (axisSpan < 0.5) return v.toFixed(2);
    if (axisSpan < 5) return v.toFixed(1);
    return v.toFixed(0);
  }
  if (v >= 1000) return (v / 1000).toFixed(1) + "k";
  if (axisSpan < 5) return v.toFixed(1);
  return v.toFixed(0);
}

function shortDayLabel(day) {
  return day.length >= 10 ? day.slice(5) : day;
}

function parseDayKey(day) {
  var p = day.slice(0, 10).split("-");
  return new Date(+p[0], +p[1] - 1, +p[2]);
}

function formatDayKey(d) {
  var y = d.getFullYear();
  var m = d.getMonth() + 1;
  var dd = d.getDate();
  return y + "-" + (m < 10 ? "0" : "") + m + "-" + (dd < 10 ? "0" : "") + dd;
}

function addDays(d, n) {
  var r = new Date(d.getTime());
  r.setDate(r.getDate() + n);
  return r;
}

function expandPointsTimeline(points, slots) {
  if (!points.length) return [];
  var latest = points[points.length - 1].day.slice(0, 10);
  var latestDate = parseDayKey(latest);
  var byDay = {};
  points.forEach(function(p) {
    byDay[p.day.slice(0, 10)] = p.v;
  });
  var out = [];
  for (var i = 0; i < slots; i++) {
    var day = formatDayKey(addDays(latestDate, -((slots - 1) - i)));
    out.push({ day: day, v: byDay[day] != null ? byDay[day] : null });
  }
  return out;
}

function sparkXDateLabel(day, slots) {
  return slots > 7 ? shortDayLabel(day) : shortDate(day);
}

function makeSparkSvg(w, h, points, color, opts) {
  if (typeof opts === "boolean") opts = { showDots: opts };
  opts = opts || {};
  var showDots = opts.showDots;
  var labeled = opts.labeled;
  var yUnit = opts.yUnit || "GiB/s";
  var xLabel = opts.xLabel || "Date";
  var timelineDays = opts.timelineDays || 0;

  var pad = labeled
    ? { t: 10, r: 14, b: timelineDays ? 72 : 64, l: 52 }
    : { t: 4, r: 3, b: 4, l: 3 };
  var plotW = w - pad.l - pad.r;
  var plotH = h - pad.t - pad.b;
  var plotL = pad.l;
  var plotT = pad.t;
  var plotB = pad.t + plotH;

  var vals = points.map(function(p) { return p.v; }).filter(function(v) { return v != null; });
  if (!vals.length) {
    var empty = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    empty.setAttribute("width", w);
    empty.setAttribute("height", h);
    return empty;
  }
  var dataMin = Math.min.apply(null, vals);
  var dataMax = Math.max.apply(null, vals);
  var mid = (dataMin + dataMax) / 2;
  var span = dataMax - dataMin;
  // Don't stretch tiny run-to-run noise across the full chart height.
  var minSpan = Math.max(Math.abs(mid) * 0.1, 1e-9);
  if (span < minSpan) {
    yMin = mid - minSpan / 2;
    yMax = mid + minSpan / 2;
  } else {
    yMin = dataMin;
    yMax = dataMax;
    var yPad = span * 0.08 || 0.1;
    yMin -= yPad;
    yMax += yPad;
  }
  var axisSpan = yMax - yMin;

  var n = points.length;
  var xAt = function(i) {
    if (n === 1) return plotL + plotW / 2;
    return plotL + (i / (n - 1)) * plotW;
  };
  var yAt = function(v) {
    return plotT + plotH - ((v - yMin) / (yMax - yMin)) * plotH;
  };

  var svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", w);
  svg.setAttribute("height", h);
  svg.setAttribute("viewBox", "0 0 " + w + " " + h);

  if (labeled) {
    var axis = document.createElementNS("http://www.w3.org/2000/svg", "path");
    axis.setAttribute("d", "M" + plotL + "," + plotT + " L" + plotL + "," + plotB + " L" + (plotL + plotW) + "," + plotB);
    axis.setAttribute("stroke", "#999");
    axis.setAttribute("stroke-width", "1");
    axis.setAttribute("fill", "none");
    svg.appendChild(axis);

    [0, 0.5, 1].forEach(function(frac) {
      var v = yMax - frac * (yMax - yMin);
      var y = yAt(v);
      var grid = document.createElementNS("http://www.w3.org/2000/svg", "line");
      grid.setAttribute("x1", plotL);
      grid.setAttribute("x2", plotL + plotW);
      grid.setAttribute("y1", y);
      grid.setAttribute("y2", y);
      grid.setAttribute("stroke", "#eee");
      grid.setAttribute("stroke-width", "1");
      svg.appendChild(grid);
      svgText(svg, plotL - 6, y + 3, fmtSparkY(v, yUnit, axisSpan), { anchor: "end", size: "9" });
    });

    var yTitle = svgText(svg, 14, plotT + plotH / 2, yUnit, { size: "10", fill: "#666", anchor: "middle" });
    yTitle.setAttribute("transform", "rotate(-90 14 " + (plotT + plotH / 2) + ")");

    var xIdx = timelineDays
      ? points.map(function(_, i) { return i; })
      : n <= 7
        ? points.map(function(_, i) { return i; })
        : [0, Math.floor((n - 1) / 2), n - 1];
    xIdx.forEach(function(i) {
      var tx = xAt(i);
      var ty = plotB + 6;
      svgText(svg, tx, ty, sparkXDateLabel(points[i].day, n), { anchor: "end", size: "8", rotate: -45 });
    });
    svgText(svg, plotL + plotW / 2, h - 4, xLabel, { anchor: "middle", size: "10", fill: "#666" });
  }

  var dataCoords = [];
  points.forEach(function(p, i) {
    if (p.v != null) dataCoords.push({ x: xAt(i), y: yAt(p.v), i: i });
  });

  var fillColor = "rgba(0,0,0,0.08)";
  if (color.charAt(0) === "#" && color.length >= 7) {
    var r = parseInt(color.slice(1, 3), 16);
    var g = parseInt(color.slice(3, 5), 16);
    var b = parseInt(color.slice(5, 7), 16);
    fillColor = "rgba(" + r + "," + g + "," + b + ",0.15)";
  }

  var segments = [];
  var current = [];
  dataCoords.forEach(function(c, idx) {
    if (!idx || c.i - dataCoords[idx - 1].i === 1) {
      current.push(c);
    } else {
      if (current.length) segments.push(current);
      current = [c];
    }
  });
  if (current.length) segments.push(current);

  segments.forEach(function(seg) {
    if (seg.length > 1) {
      var area = document.createElementNS("http://www.w3.org/2000/svg", "polygon");
      var areaPts = seg.map(function(c) { return c.x + "," + c.y; }).join(" ");
      areaPts += " " + seg[seg.length - 1].x + "," + plotB;
      areaPts += " " + seg[0].x + "," + plotB;
      area.setAttribute("points", areaPts);
      area.setAttribute("fill", fillColor);
      area.setAttribute("stroke", "none");
      svg.appendChild(area);

      var line = document.createElementNS("http://www.w3.org/2000/svg", "polyline");
      line.setAttribute("points", seg.map(function(c) { return c.x + "," + c.y; }).join(" "));
      line.setAttribute("stroke", color);
      line.setAttribute("stroke-width", showDots ? "2" : "1.5");
      line.setAttribute("fill", "none");
      svg.appendChild(line);
    }
  });

  if (showDots || dataCoords.length === 1) {
    dataCoords.forEach(function(c) {
      var dot = document.createElementNS("http://www.w3.org/2000/svg", "circle");
      dot.setAttribute("cx", c.x);
      dot.setAttribute("cy", c.y);
      dot.setAttribute("r", "3");
      dot.setAttribute("fill", color);
      svg.appendChild(dot);
    });
  }

  return svg;
}

function makeMultiSparkSvg(w, h, seriesList, opts) {
  opts = opts || {};
  var showDots = opts.showDots;
  var labeled = opts.labeled;
  var yUnit = opts.yUnit || "GiB/s";
  var xLabel = opts.xLabel || "Date";
  var timelineDays = opts.timelineDays || 0;

  var pad = labeled
    ? { t: 10, r: 14, b: timelineDays ? 72 : 64, l: 52 }
    : { t: 4, r: 3, b: 4, l: 3 };
  var plotW = w - pad.l - pad.r;
  var plotH = h - pad.t - pad.b;
  var plotL = pad.l;
  var plotT = pad.t;
  var plotB = pad.t + plotH;

  var vals = [];
  seriesList.forEach(function(s) {
    s.points.forEach(function(p) {
      if (p.v != null) vals.push(p.v);
    });
  });
  if (!vals.length) {
    var empty = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    empty.setAttribute("width", w);
    empty.setAttribute("height", h);
    return empty;
  }

  var yFixed = !!opts.yScaleLock;
  var yMin;
  var yMax;
  if (yFixed) {
    yMin = opts.yMin;
    yMax = opts.yMax;
  } else {
    var dataMin = Math.min.apply(null, vals);
    var dataMax = Math.max.apply(null, vals);
    var mid = (dataMin + dataMax) / 2;
    var span = dataMax - dataMin;
    var minSpan = Math.max(Math.abs(mid) * 0.1, 1e-9);
    if (span < minSpan) {
      yMin = mid - minSpan / 2;
      yMax = mid + minSpan / 2;
    } else {
      yMin = dataMin;
      yMax = dataMax;
      var yPad = span * 0.08 || 0.1;
      yMin -= yPad;
      yMax += yPad;
    }
  }
  var axisSpan = yMax - yMin;

  var refPoints = seriesList[0].points;
  var n = refPoints.length;
  var xAt = function(i) {
    if (n === 1) return plotL + plotW / 2;
    return plotL + (i / (n - 1)) * plotW;
  };
  var yAt = function(v) {
    var frac = (v - yMin) / (yMax - yMin);
    if (yFixed) frac = Math.max(0, Math.min(1, frac));
    return plotT + plotH - frac * plotH;
  };

  var svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("width", w);
  svg.setAttribute("height", h);
  svg.setAttribute("viewBox", "0 0 " + w + " " + h);

  if (labeled) {
    var axis = document.createElementNS("http://www.w3.org/2000/svg", "path");
    axis.setAttribute("d", "M" + plotL + "," + plotT + " L" + plotL + "," + plotB + " L" + (plotL + plotW) + "," + plotB);
    axis.setAttribute("stroke", "#999");
    axis.setAttribute("stroke-width", "1");
    axis.setAttribute("fill", "none");
    svg.appendChild(axis);

    var yTickValues = yFixed && opts.yTicks
      ? opts.yTicks
      : [0, 0.5, 1].map(function(frac) { return yMax - frac * (yMax - yMin); });
    yTickValues.forEach(function(v) {
      var y = yAt(v);
      var grid = document.createElementNS("http://www.w3.org/2000/svg", "line");
      grid.setAttribute("x1", plotL);
      grid.setAttribute("x2", plotL + plotW);
      grid.setAttribute("y1", y);
      grid.setAttribute("y2", y);
      grid.setAttribute("stroke", "#eee");
      grid.setAttribute("stroke-width", "1");
      svg.appendChild(grid);
      svgText(svg, plotL - 6, y + 3, fmtSparkY(v, yUnit, axisSpan), { anchor: "end", size: "9" });
    });

    var yTitle = svgText(svg, 14, plotT + plotH / 2, yUnit, { size: "10", fill: "#666", anchor: "middle" });
    yTitle.setAttribute("transform", "rotate(-90 14 " + (plotT + plotH / 2) + ")");

    var xIdx = timelineDays
      ? refPoints.map(function(_, i) { return i; })
      : n <= 7
        ? refPoints.map(function(_, i) { return i; })
        : [0, Math.floor((n - 1) / 2), n - 1];
    xIdx.forEach(function(i) {
      var tx = xAt(i);
      var ty = plotB + 6;
      svgText(svg, tx, ty, sparkXDateLabel(refPoints[i].day, n), { anchor: "end", size: "8", rotate: -45 });
    });
    svgText(svg, plotL + plotW / 2, h - 4, xLabel, { anchor: "middle", size: "10", fill: "#666" });
  }

  seriesList.forEach(function(s) {
    var dataCoords = [];
    s.points.forEach(function(p, i) {
      if (p.v != null) dataCoords.push({ x: xAt(i), y: yAt(p.v), i: i });
    });

    var segments = [];
    var current = [];
    dataCoords.forEach(function(c, idx) {
      if (!idx || c.i - dataCoords[idx - 1].i === 1) {
        current.push(c);
      } else {
        if (current.length) segments.push(current);
        current = [c];
      }
    });
    if (current.length) segments.push(current);

    segments.forEach(function(seg) {
      if (seg.length > 1) {
        var line = document.createElementNS("http://www.w3.org/2000/svg", "polyline");
        line.setAttribute("points", seg.map(function(c) { return c.x + "," + c.y; }).join(" "));
        line.setAttribute("stroke", s.color);
        line.setAttribute("stroke-width", showDots ? "2" : "1.5");
        line.setAttribute("fill", "none");
        if (s.dash) line.setAttribute("stroke-dasharray", s.dash);
        svg.appendChild(line);
      }
    });

    if (showDots || dataCoords.length === 1) {
      dataCoords.forEach(function(c) {
        var dot = document.createElementNS("http://www.w3.org/2000/svg", "circle");
        dot.setAttribute("cx", c.x);
        dot.setAttribute("cy", c.y);
        dot.setAttribute("r", "3");
        dot.setAttribute("fill", s.color);
        svg.appendChild(dot);
      });
    }
  });

  return svg;
}

function perCacheLineNs(passNs) {
  return passNs / CACHE_LINES_PER_GI;
}

function hasMachineData(m) {
  return TABLE.some(function(sec) {
    return sec.rows.some(function(row) { return hasOp(m, row.key); });
  });
}

function machineFromHash(machines) {
  var match = (location.hash || "").match(/^#?m=(.+)$/);
  if (!match) return null;
  var key = decodeURIComponent(match[1]);
  return machines.indexOf(key) >= 0 ? key : null;
}

function partitionByCloud(machines) {
  var gcp = [];
  var aws = [];
  var other = [];
  machines.forEach(function(m) {
    if (m.indexOf("gcp-") === 0) gcp.push(m);
    else if (m.indexOf("aws-") === 0) aws.push(m);
    else other.push(m);
  });
  gcp.sort();
  aws.sort();
  other.sort();
  return { gcp: gcp, aws: aws, other: other };
}

function gcpVcpuTier(id) {
  var m = id.match(/standard-(\d+)/);
  return m ? parseInt(m[1], 10) : 0;
}

function awsSizeTier(id) {
  var m = id.match(/\.(\d+)xlarge$/);
  return m ? parseInt(m[1], 10) : 0;
}

function groupByTier(list, tierFn) {
  var groups = {};
  list.forEach(function(key) {
    var tier = tierFn(key);
    if (!groups[tier]) groups[tier] = [];
    groups[tier].push(key);
  });
  return groups;
}

function uniqueTiers(list, tierFn) {
  var seen = {};
  list.forEach(function(key) {
    var tier = tierFn(key);
    if (tier > 0) seen[tier] = true;
  });
  return Object.keys(seen).map(function(t) { return parseInt(t, 10); }).sort(function(a, b) { return a - b; });
}

function buildSizeColumns(container, list, tiers, tierFn, labelFn, state, onSelect) {
  var row = document.createElement("div");
  row.className = "picker-lattice-row cols-" + tiers.length;
  var byTier = groupByTier(list, tierFn);
  tiers.forEach(function(tier) {
    var col = document.createElement("div");
    col.className = "picker-lattice-col";
    (byTier[tier] || []).sort().forEach(function(key) {
      col.appendChild(makePickerCell(labelFn(key), key, state, onSelect));
    });
    row.appendChild(col);
  });
  container.appendChild(row);
}

function buildPickerLattice(container, parts, state, onSelect) {
  if (!container) return;
  container.innerHTML = "";
  if (parts.gcp.length) {
    var gcpHeading = document.createElement("div");
    gcpHeading.className = "picker-section";
    gcpHeading.textContent = "GCP";
    container.appendChild(gcpHeading);
    buildSizeColumns(container, parts.gcp, uniqueTiers(parts.gcp, gcpVcpuTier), gcpVcpuTier, shortMachine, state, onSelect);
  }
  if (parts.aws.length) {
    var awsHeading = document.createElement("div");
    awsHeading.className = "picker-section";
    awsHeading.textContent = "AWS";
    container.appendChild(awsHeading);
    buildSizeColumns(container, parts.aws, uniqueTiers(parts.aws, awsSizeTier), awsSizeTier, shortAws, state, onSelect);
  }
}

function setPickerOpen(menu, toggle, open) {
  menu.classList.toggle("open", open);
  toggle.setAttribute("aria-expanded", open ? "true" : "false");
}

function initMachinePicker(machines, state, onSelect) {
  var picker = document.getElementById("machine-picker");
  var trigger = document.getElementById("picker-trigger");
  var menu = document.getElementById("picker-menu");
  var lattice = document.getElementById("picker-lattice");
  var listOther = document.getElementById("picker-list-other");
  var otherHeading = document.getElementById("picker-other-heading");
  if (!picker || !trigger || !menu || !lattice || !listOther || !otherHeading) return;

  listOther.innerHTML = "";
  var parts = partitionByCloud(machines);

  buildPickerLattice(lattice, parts, state, onSelect);
  lattice.hidden = !parts.gcp.length && !parts.aws.length;

  if (parts.other.length) {
    otherHeading.hidden = false;
    parts.other.forEach(function(key) {
      listOther.appendChild(makePickerCell(pickerLabel(key), key, state, onSelect));
    });
  } else {
    otherHeading.hidden = true;
  }

  setPickerLabel(state.selected);

  trigger.addEventListener("click", function(e) {
    e.stopPropagation();
    setPickerOpen(menu, trigger, !menu.classList.contains("open"));
  });

  document.addEventListener("mousedown", function(e) {
    if (!menu.classList.contains("open")) return;
    if (!picker.contains(e.target)) setPickerOpen(menu, trigger, false);
  });
}

function makePickerCell(label, key, state, onSelect) {
  var row = document.createElement("div");
  row.className = "picker-cell available";
  row.dataset.machine = key;
  row.setAttribute("role", "option");
  if (key === state.selected) row.classList.add("active");

  var box = document.createElement("input");
  box.type = "checkbox";
  box.className = "picker-check";
  box.tabIndex = -1;
  box.checked = key === state.selected;

  row.appendChild(box);
  row.appendChild(document.createTextNode(label));

  row.addEventListener("mousedown", function(e) {
    e.preventDefault();
    e.stopPropagation();
  });

  row.addEventListener("click", function(e) {
    e.stopPropagation();
    state.selected = key;
    setPickerLabel(key);
    updatePickerActiveFromState(state);
    onSelect(key);
  });

  return row;
}

function setPickerLabel(key) {
  document.getElementById("picker-label").textContent = pickerLabel(key);
}

function pickerLabel(id) {
  return id.replace(/^gcp-/, "").replace(/^aws-/, "");
}

function updatePickerActiveFromState(state) {
  document.querySelectorAll(".picker-cell.available").forEach(function(row) {
    var on = row.dataset.machine === state.selected;
    row.classList.toggle("active", on);
    var box = row.querySelector(".picker-check");
    if (box) box.checked = on;
  });
}

function sectionSlug(title) {
  return "sec-" + title.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}

function tocLabel(title) {
  return title.replace(/\s*\([^)]*\)\s*$/, "").trim();
}

function tocGroup(title) {
  var base = tocLabel(title);
  var comma = base.indexOf(",");
  if (comma >= 0) return base.slice(0, comma).trim();

  var tag = (title.match(/\(([^)]+)\)\s*$/) || [])[1];
  if (tag === "bincode" || tag === "JSON" || tag === "LZ4") return tag;

  return base;
}

function tocGroups(m) {
  var groups = [];
  var seen = {};
  TABLE.forEach(function(sec) {
    var present = sec.rows.some(function(row) { return hasOp(m, row.key); });
    if (!present) return;
    var label = tocGroup(sec.title);
    var sectionId = sectionSlug(sec.title);
    if (!seen[label]) {
      seen[label] = { label: label, sectionId: sectionId, sectionIds: [sectionId] };
      groups.push(seen[label]);
    } else {
      seen[label].sectionIds.push(sectionId);
    }
  });
  return groups;
}

var tocSpyBound = false;

function ensureTocSpy() {
  if (tocSpyBound) return;
  tocSpyBound = true;
  window.addEventListener("scroll", syncTocActive, { passive: true });
}

function sectionHead(tbody) {
  if (!tbody) return null;
  return tbody.querySelector(".section-head") || tbody.rows[0] || null;
}

function tocMarkerY() {
  var v = getComputedStyle(document.documentElement).getPropertyValue("--scroll-marker-y");
  return v ? parseFloat(v) : 96;
}

function contentBandTop() {
  var header = document.querySelector(".page-header");
  if (!header) return tocMarkerY();
  return header.getBoundingClientRect().bottom;
}

function sectionInContentBand(tbody) {
  var rect = tbody.getBoundingClientRect();
  var top = contentBandTop();
  return rect.bottom > top && rect.top < window.innerHeight;
}

function visibleSectionIds() {
  var ids = [];
  document.querySelectorAll("#bench-table tbody[id]").forEach(function(tbody) {
    if (sectionInContentBand(tbody)) ids.push(tbody.id);
  });
  return ids;
}

function updateStickyChrome() {
  var header = document.querySelector(".page-header");
  if (!header) return;
  var chromeTop = header.offsetHeight;
  document.documentElement.style.setProperty("--sticky-chrome-top", chromeTop + "px");
  document.documentElement.style.setProperty("--scroll-marker-y", chromeTop + "px");
  updateTocIndicator();
}

function tocMarkerSlack() {
  return 12;
}

function sectionDocTop(el) {
  return el.getBoundingClientRect().top + window.pageYOffset;
}

function scrollToSection(tbody) {
  var head = sectionHead(tbody) || tbody;
  var y = Math.max(0, Math.round(sectionDocTop(head) - contentBandTop() - tocMarkerSlack()));
  window.scrollTo(0, y);
}

function updateTocIndicator() {
  var nav = document.getElementById("bench-toc");
  var bar = document.getElementById("bench-toc-indicator");
  if (!nav || !bar || nav.hidden) return;

  var visible = {};
  visibleSectionIds().forEach(function(id) { visible[id] = true; });

  var showing = [];
  nav.querySelectorAll("a[data-section-ids]").forEach(function(a) {
    var ids = a.dataset.sectionIds.split(" ");
    for (var i = 0; i < ids.length; i++) {
      if (visible[ids[i]]) {
        showing.push(a);
        break;
      }
    }
  });

  if (!showing.length) {
    bar.hidden = true;
    return;
  }

  var top = showing[0].offsetTop;
  var last = showing[showing.length - 1];
  bar.style.top = top + "px";
  bar.style.height = (last.offsetTop + last.offsetHeight - top) + "px";
  bar.hidden = false;
}

var tocSpyBound = false;

function ensureTocIndicator(nav) {
  if (!nav) return;
  if (!document.getElementById("bench-toc-indicator")) {
    var bar = document.createElement("div");
    bar.id = "bench-toc-indicator";
    bar.className = "page-toc-indicator";
    bar.hidden = true;
    nav.appendChild(bar);
  }
}

function syncTocActive() {
  updateTocIndicator();
}

function renderToc(m, nav) {
  var stack = document.querySelector(".page-stack");
  if (!nav) return;
  nav.innerHTML = "";
  var groups = tocGroups(m);
  groups.forEach(function(g) {
    var a = document.createElement("a");
    a.href = "#" + g.sectionId;
    a.dataset.sectionIds = g.sectionIds.join(" ");
    a.textContent = g.label;
    a.addEventListener("click", function(e) {
      e.preventDefault();
      hidePopover();
      var tbody = document.getElementById(g.sectionId);
      if (tbody) scrollToSection(tbody);
    });
    nav.appendChild(a);
  });
  nav.hidden = groups.length === 0;
  ensureTocIndicator(nav);
  if (stack) stack.classList.toggle("has-toc", groups.length > 0);
  ensureTocSpy();
  requestAnimationFrame(function() {
    syncTocActive();
    updateTocIndicator();
  });
}

function shortAws(id) {
  return id.replace(/^aws-/, "");
}

function shortMachine(id) {
  return id
    .replace(/^gcp-/, "")
    .replace(/^aws-/, "")
    .replace(/-standard-/, "-")
    .replace(/-lssd$/, "");
}

function dayKey(date) {
  return date.length >= 10 ? date.slice(0, 10) : date;
}

function shortDate(d) {
  return d.length >= 10 ? d.slice(0, 10) : d;
}

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
  if (ns >= 1e9) return (ns / 1e9).toFixed(0) + " s";
  if (ns >= 1e6) return (ns / 1e6).toFixed(0) + " ms";
  if (ns >= 1e3) return (ns / 1e3).toFixed(0) + " μs";
  return ns.toFixed(1) + " ns";
}

function fmtThroughput(v) {
  if (v >= 1073741824) return (v / 1073741824).toFixed(0) + " GiB/s";
  if (v >= 1048576) return (v / 1048576).toFixed(0) + " MiB/s";
  return (v / 1024).toFixed(0) + " KiB/s";
}

function fmtTime(secs) {
  if (secs >= 1) return (secs >= 60 ? (secs / 60).toFixed(0) + "m" : secs.toFixed(0) + " s");
  if (secs >= 0.001) return (secs * 1000).toFixed(0) + " ms";
  return (secs * 1e6).toFixed(0) + " μs";
}

if (window.__NAPKIN_DATA__ && Array.isArray(window.__NAPKIN_DATA__.rows)) {
  boot(window.__NAPKIN_DATA__.rows, window.__NAPKIN_DATA__.changelog);
} else {
  Promise.all([
    fetch("data/data.csv", { cache: "no-store" }).then(function(r) {
      if (!r.ok) throw new Error("data.csv HTTP " + r.status);
      return r.text();
    }),
    fetch("data/changelog.json", { cache: "no-store" })
      .then(function(r) { return r.ok ? r.json() : []; })
      .catch(function() { return []; })
  ]).then(function(res) {
    boot(parseCSV(res[0]), res[1]);
  }).catch(fail);
}
