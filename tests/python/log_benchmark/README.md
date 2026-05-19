# Python SDK logging benchmarks

Manual performance benchmarks for the Rerun Python SDK logging pipeline.
These are **not** run in CI — they are intended for local profiling and regression checks.

## Running benchmarks

From the `rerun/` directory:

```bash
# Run all benchmarks:
pixi run py-bench

# Run only throughput benchmarks:
pixi run py-bench -k "not micro"

# Run only micro-benchmarks:
pixi run py-bench -k micro

# Run a specific benchmark:
pixi run py-bench -k "micro_log-Points3D"
```

## Running standalone (for profiling)

Enter the pixi shell first:

```bash
pixi shell
```

Then run a benchmark directly:

```bash
# Run the throughput benchmark standalone:
uvpy -m tests.python.log_benchmark.test_log_benchmark transform3d

# With options:
uvpy -m tests.python.log_benchmark.test_log_benchmark transform3d --num-entities 10 --num-time-steps 10000 --static

# Connect to a running Rerun viewer (start `rerun` first):
uvpy -m tests.python.log_benchmark.test_log_benchmark transform3d --connect
```

### Profiling with py-spy

```bash
# Generate a flamegraph (on Linux, add --native for native stack traces):
sudo PYTHONPATH=rerun_py/rerun_sdk:rerun_py py-spy record -o flamegraph.svg -- \
    .venv/bin/python -m tests.python.log_benchmark.test_log_benchmark transform3d

# Then open flamegraph.svg in a browser
```

## Comparing benchmark runs

Use `--benchmark-save` to save benchmark results:

```bash
# Save a baseline on the current branch:
pixi run py-bench -k micro --benchmark-save=before

# Make changes, rebuild, then save again:
pixi run py-bench -k micro --benchmark-save=after
```

Saved results are stored in `.benchmarks/` under the project root and are automatically numbered, e.g. `0001_before` and `0002_after`.

You can then compare using the `pytest-benchmark` CLI:
```
uv run pytest-benchmark compare 0001 0002
```


## Test files

- `__init__.py` — Shared data classes (`Point3DInput`, `Transform3DInput`)
- `test_log_benchmark.py` — Throughput benchmarks
- `test_micro_benchmark.py` — Per-call overhead micro-benchmarks
