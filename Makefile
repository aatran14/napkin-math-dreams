.PHONY: publish bench-local

# Merge per-date CSVs and regenerate index.html with embedded data.
publish:
	python3 scripts/publish.py

# Run the bench fleet locally against this checkout (no GitHub Actions).
bench-local:
	bash scripts/run-bench-local.sh
