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
| H6 | Bypass PyArrow (Rust endpoint) | Not tested | - | Deferred |

## Final Results (all optimizations applied)

| Metric | Baseline | Final | Improvement |
|--------|----------|-------|-------------|
| Full log | 36.8k xforms/s | 45.2k xforms/s | **+22%** |
| Create only | 112k xforms/s | 121k xforms/s | **+8%** |

## Summary of Changes

### Rust (requires rebuild)
- `rerun_py/src/arrow.rs`: Fast-path downcast of `PyComponentDescriptor` to avoid 3 getattr calls per descriptor

### Python (no rebuild needed)
- `rerun_py/rerun_sdk/rerun/error_utils.py`: Replace `getattr(threading.local(), ...)` with direct `__dict__` access throughout `catch_and_log_exceptions`, `strict_mode()`, and `_send_warning_or_raise()`
- `rerun_py/rerun_sdk/rerun/_baseclasses.py`: Cache `ComponentDescriptor` objects and pre-compute component field name lists per archetype class in `as_component_batches()`
- `rerun_py/rerun_sdk/rerun/datatypes/mat3x3_ext.py`: Fast-path numpy array handling in `native_to_pa_array_override()` that skips `Mat3x3` object creation

## H6 Assessment

H6 (bypass PyArrow, build Arrow arrays in Rust) was deferred. The 22% cumulative improvement from H1-H5 is meaningful but still leaves substantial room for improvement. The remaining bottlenecks are:
1. PyArrow `FixedSizeListArray.from_arrays()` overhead for small arrays (3 and 9 floats)
2. Python-level iteration over entities/timesteps (100k Python loop iterations)
3. `attrs` converter overhead (calling `_converter` for each field, 6 of which return None)

H6 would help with #1 but requires significantly more effort (~100 lines of Rust). The `send_columns` API is the better path for high-throughput logging since it amortizes per-call overhead.
