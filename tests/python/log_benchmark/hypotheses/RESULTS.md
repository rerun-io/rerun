# Python Logging Performance Investigation Results

## Benchmark: `log_transform3d_translation_mat3x3` (100 entities x 1000 time steps)

All measurements on release builds (`pixi run py-build-release`).

## Baseline

| Metric | Performance |
|--------|-------------|
| Full log (construct + log) | 36.8k transforms/s |
| Create only (construct) | 112k transforms/s |

## Individual Hypothesis Results

| # | Hypothesis | Change | Impact | Decision |
|---|-----------|--------|--------|----------|
| H1 | Downcast PyComponentDescriptor in Rust | +1.4% | Negligible | KEEP (zero-risk, 3 lines) |
| H2 | Optimize threading.local access via `__dict__` | +10% | Significant | KEEP |
| H3+H5 | Cache descriptors + pre-compute component fields | +5% (cumulative) | Moderate | KEEP |
| H4 | Streamline Mat3x3 numpy ops (skip object creation) | +7.5% (cumulative) | Moderate | KEEP |
| H7 | Bypass PyArrow — build Arrow FixedSizeListArrays in Rust | ~11% (of remaining) | Significant | KEEP |
| H8 | Inline try/except instead of catch_and_log_exceptions | ~5% (of remaining) | Moderate | KEEP |
| H9 | Streamline _log_components dict building | ~5% (of remaining) | Moderate | KEEP |

## Cumulative Results

### After H1-H4 (Round 1)

| Metric | Baseline | After H1-H4 | Improvement |
|--------|----------|-------------|-------------|
| Full log | 36.8k xforms/s | 45.2k xforms/s | **+22%** |
| Create only | 112k xforms/s | 121k xforms/s | **+8%** |

### After H7-H9 (Round 2)

| Metric | After H1-H4 | After H7-H9 | Improvement |
|--------|-------------|-------------|-------------|
| Full log | 45.2k xforms/s | 73.0k xforms/s | **+62%** |
| Micro-benchmark (full pipeline) | ~17.5 us/call | ~12.0 us/call | **-31%** |

### Overall (Baseline to Final)

| Metric | Baseline | Final | Improvement |
|--------|----------|-------|-------------|
| Full log | 36.8k xforms/s | 73.0k xforms/s | **+98%** (~2x) |

## Summary of Changes

### Rust (requires rebuild)
- `rerun_py/src/arrow.rs`: Fast-path downcast of `PyComponentDescriptor` (H1); new `build_fixed_size_list_array()` function that builds Arrow FixedSizeListArrays directly from numpy, bypassing PyArrow overhead (H7)
- `rerun_py/src/python_bridge.rs`: Register `build_fixed_size_list_array` as pyfunction (H7)

### Python (no rebuild needed)
- `rerun_py/rerun_sdk/rerun/error_utils.py`: Replace `getattr(threading.local(), ...)` with direct `__dict__` access (H2)
- `rerun_py/rerun_sdk/rerun/_baseclasses.py`: Cache `ComponentDescriptor` objects and pre-compute component field name lists (H3+H5); inline `try/except` in `BaseBatch.__init__` replacing `catch_and_log_exceptions` context manager (H8)
- `rerun_py/rerun_sdk/rerun/datatypes/mat3x3_ext.py`: Fast-path numpy array handling (H4); use Rust arrow builder (H7)
- `rerun_py/rerun_sdk/rerun/datatypes/vec3d_ext.py`: Use Rust arrow builder (H7)
- `rerun_py/rerun_sdk/rerun/archetypes/transform3d_ext.py`: Inline `try/except` replacing `catch_and_log_exceptions` context manager (H8)
- `rerun_py/rerun_sdk/rerun/_log.py`: Single dict comprehension in `_log_components` (H9)

## Remaining Time Budget (~12.0 us/call)

```
Full call: ~12.0 us  (excluding 2x set_time @ 0.25 us each)
+-- Transform3D construction:              5.0 us  (42%)
|   +-- Translation3DBatch:                1.8 us
|   +-- TransformMat3x3Batch:              2.2 us
|   +-- attrs overhead (converters etc):   ~1.0 us
+-- rr.log() Python overhead:              3.5 us  (29%)
|   +-- catch_and_log_exceptions (log):    0.29 us
|   +-- as_component_batches():            0.55 us
|   +-- _log_components:                   3.0 us
|       +-- dict comprehension:            0.2 us
|       +-- bindings.log_arrow_msg:        2.6 us  (Rust FFI)
+-- Remaining glue:                        ~3.5 us  (29%)
```

Further optimization would likely require either:
1. Moving more of the construction path to Rust (bypassing attrs converters)
2. Using the `send_columns` API to amortize per-call overhead across many rows
3. Batch-aware entity logging that avoids Python-level iteration
