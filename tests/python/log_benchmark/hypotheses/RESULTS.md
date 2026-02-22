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
| H10 | NativeArrowArray — bypass PyArrow on logging hot path | ~85% (of remaining) | **Major** | KEEP |

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

### After H10 (Round 3)

| Metric | After H7-H9 | After H10 | Improvement |
|--------|-------------|-----------|-------------|
| Full log | 73.0k xforms/s | 135k xforms/s | **+85%** |
| Micro-benchmark (full pipeline) | ~12.0 us/call | ~5.98 us/call | **-50%** |

### Overall (Baseline to Final)

| Metric | Baseline | Final | Improvement |
|--------|----------|-------|-------------|
| Full log | 36.8k xforms/s | **135k xforms/s** | **+267%** (~3.7x) |
| Micro-benchmark (full pipeline) | ~27 us/call | ~5.98 us/call | **-78%** |

## Summary of Changes

### Rust (requires rebuild)
- `rerun_py/src/arrow.rs`: Fast-path downcast of `PyComponentDescriptor` (H1); `build_fixed_size_list_array()` function that builds Arrow FixedSizeListArrays directly from numpy (H7); `NativeArrowArray` pyclass that keeps Arrow data on the Rust side as `Arc<dyn Array>` (H10); fast-path `array_to_rust()` that clones the Arc instead of PyArrow FFI round-trip (H10)
- `rerun_py/src/python_bridge.rs`: Register `build_fixed_size_list_array` (H7) and `NativeArrowArray` class (H10)

### Python (no rebuild needed)
- `rerun_py/rerun_sdk/rerun/error_utils.py`: Replace `getattr(threading.local(), ...)` with direct `__dict__` access (H2)
- `rerun_py/rerun_sdk/rerun/_baseclasses.py`: Cache `ComponentDescriptor` objects and pre-compute component field name lists (H3+H5); inline `try/except` in `BaseBatch.__init__` replacing `catch_and_log_exceptions` context manager (H8); lazy `NativeArrowArray → pa.Array` conversion via `__getattr__` (H10)
- `rerun_py/rerun_sdk/rerun/datatypes/mat3x3_ext.py`: Fast-path numpy array handling (H4); use Rust arrow builder (H7)
- `rerun_py/rerun_sdk/rerun/datatypes/vec3d_ext.py`: Use Rust arrow builder (H7)
- `rerun_py/rerun_sdk/rerun/archetypes/transform3d_ext.py`: Inline `try/except` replacing `catch_and_log_exceptions` context manager (H8)
- `rerun_py/rerun_sdk/rerun/_log.py`: Single dict comprehension in `_log_components` (H9); inline `catch_and_log_exceptions` on `log()` (H10); hot-path `_native_array` access bypassing PyArrow (H10)

## Time Budget: ~5.98 us/call (after H10)

```
Full rr.log("path", rr.Transform3D(translation=list, mat3x3=np)):  ~5.98 us
│
├── Transform3D construction:                               2.46 us  (41%)
│   ├── Translation3DBatch(list):                           0.73 us
│   │   ├── flat_np_float32_array_from_array_like:          0.31 us
│   │   ├── build_fixed_size_list_array (Rust):             0.20 us
│   │   └── BaseBatch overhead (isinstance, etc):           0.22 us
│   ├── TransformMat3x3Batch(np.eye(3)):                    1.03 us
│   │   ├── numpy reshape+transpose+ravel:                  0.45 us
│   │   ├── build_fixed_size_list_array (Rust):             0.21 us
│   │   └── BaseBatch overhead:                             0.37 us
│   └── attrs overhead (6x _converter(None)):               0.20 us
│       └── __attrs_init__ remaining glue:                  0.50 us
│
├── rr.log() overhead:                                      1.75 us  (29%)
│   ├── hasattr + as_component_batches():                   0.57 us
│   │   ├── hasattr check:                                  0.03 us
│   │   └── as_component_batches():                         0.54 us
│   ├── _log_components:                                    0.86 us
│   │   ├── build instanced dict (getattr + descriptors):   0.24 us
│   │   └── bindings.log_arrow_msg (Rust FFI):              0.62 us
│   │       ├── descriptor_to_rust (2x):                    ~0.1 us
│   │       ├── array_to_rust (2x, Arc clone):              ~0.1 us
│   │       └── PendingRow + RowId + recording:             ~0.4 us
│   └── try/except + list():                                0.32 us
│
└── set_time overhead (2x, outside log):                    0.50 us  (not included above)
    └── rr.set_time('frame', sequence=42):                  0.25 us each
```

## Key Optimization Insights

1. **NativeArrowArray (H10)** was the single biggest win: keeping data as `Arc<dyn Array>` on the Rust side eliminates ~2 us of PyArrow FFI round-trip per component (export `.to_data()` + import `make_array()`). For Transform3D with 2 components, this saves ~4 us.

2. **Construction dominates** (41% of total time). The attrs converter machinery, numpy operations, and `flat_np_float32_array_from_array_like` are now the main bottleneck. Further gains would require moving the construction path to Rust.

3. **The Rust FFI boundary is cheap** when using NativeArrowArray. `bindings.log_arrow_msg` with Arc-cloned arrays costs only ~0.62 us — down from ~2.6 us with PyArrow arrays.

4. **Lazy conversion** (`__getattr__` for `.pa_array`) gives us the best of both worlds: hot path keeps NativeArrowArray opaque, cold paths (compound types, `__str__`, tests) get a real `pa.Array` on first access with automatic caching.

## Further Optimization Opportunities

At ~6 us per log call, remaining gains would likely require:
1. **Move batch construction to Rust** — the attrs converters + numpy prep account for ~2.5 us
2. **Bulk logging API** — `send_columns` amortizes per-call overhead across many rows
3. **Pre-built archetype caching** — if the same archetype is logged repeatedly with different data, cache the archetype shell
