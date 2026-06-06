#!/usr/bin/env python3
"""Merge per-date benchmark CSVs and publish index.html with embedded data.

Each data/YYYY-MM-DD.csv is self-describing (carries its own header), so the
schema can grow over time without breaking older files. We union the columns
across all files and bake one JSON payload into index.html, so the published
site needs no server and no live fetch.
"""

import csv
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DATA_DIR = ROOT / "data"
TEMPLATE = ROOT / "web" / "index.template.html"
OUTPUT = ROOT / "index.html"
CHANGELOG = DATA_DIR / "changelog.json"
INJECT_MARKER = "<!-- INJECT_DATA -->"


def read_daily():
    """Read every per-date file with its own header (schema-tolerant)."""
    cols, seen, rows = [], set(), []
    for path in sorted(DATA_DIR.glob("2*.csv")):
        text = path.read_text().strip()
        if not text:
            continue
        reader = csv.DictReader(text.splitlines())
        for field in reader.fieldnames or []:
            if field not in seen:
                seen.add(field)
                cols.append(field)
        rows.extend(reader)
    return cols, rows


def write_merged(cols, rows):
    with (DATA_DIR / "data.csv").open("w", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=cols, extrasaction="ignore")
        writer.writeheader()
        for row in rows:
            writer.writerow({c: row.get(c, "") for c in cols})


def load_changelog():
    if not CHANGELOG.is_file():
        return []
    try:
        return json.loads(CHANGELOG.read_text())
    except json.JSONDecodeError as exc:
        raise SystemExit(f"Invalid {CHANGELOG}: {exc}")


def inject(html, payload):
    blob = json.dumps(payload, separators=(",", ":"))
    script = f"<script>window.__NAPKIN_DATA__={blob};</script>"
    if INJECT_MARKER not in html:
        raise SystemExit(f"Missing {INJECT_MARKER} in {TEMPLATE}")
    return html.replace(INJECT_MARKER, script)


def main():
    if not TEMPLATE.is_file():
        raise SystemExit(f"Missing template: {TEMPLATE}")
    cols, rows = read_daily()
    write_merged(cols, rows)
    payload = {"rows": rows, "changelog": load_changelog()}
    OUTPUT.write_text(inject(TEMPLATE.read_text(), payload))
    print(
        f"Published {OUTPUT} ({len(rows)} rows, {len(cols)} cols, "
        f"{len(payload['changelog'])} changelog entries)"
    )


if __name__ == "__main__":
    main()
