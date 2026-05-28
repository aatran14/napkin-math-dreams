fetch("data/dead.csv", { cache: "no-store" })
  .then(function(r) { return r.text(); })
  .then(function(csvText) {
    var lines = csvText.split(/\r?\n/).filter(function(l) { return l.trim(); });
    var headers = lines[0].split(",");

    var table = document.createElement("table");

    var thead = document.createElement("thead");
    var headerRow = document.createElement("tr");
    headers.forEach(function(h) {
      var th = document.createElement("th");
      th.textContent = h.trim();
      headerRow.appendChild(th);
    });
    thead.appendChild(headerRow);
    table.appendChild(thead);

    var tbody = document.createElement("tbody");
    for (var i = 1; i < lines.length; i++) {
      var cells = lines[i].split(",");
      var tr = document.createElement("tr");
      cells.forEach(function(c, j) {
        var td = document.createElement("td");
        var val = c.trim();
        if (headers[j].trim() === "throughput_bytes_s" && val) {
          td.textContent = formatThroughput(parseFloat(val));
        } else if (headers[j].trim() === "latency_ns" && val) {
          td.textContent = formatLatency(parseFloat(val));
        } else {
          td.textContent = val;
        }
        tr.appendChild(td);
      });
      tbody.appendChild(tr);
    }
    table.appendChild(tbody);

    document.getElementById("charts").appendChild(table);
  });

function formatThroughput(v) {
  if (v >= 1073741824) return (v / 1073741824).toFixed(1) + " GiB/s";
  if (v >= 1048576) return (v / 1048576).toFixed(1) + " MiB/s";
  return (v / 1024).toFixed(1) + " KiB/s";
}

function formatLatency(v) {
  if (v >= 1000000) return (v / 1000000).toFixed(1) + " ms";
  if (v >= 1000) return (v / 1000).toFixed(1) + " us";
  return v.toFixed(1) + " ns";
}
