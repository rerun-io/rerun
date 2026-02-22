# Hypothesis 10: NativeArrowArray — bypass PyArrow on the logging hot path

## Hypothesis
After H1-H9, the logging hot path round-trips Arrow data through PyArrow needlessly: `build_fixed_size_list_array` exports to `PyArrowType<ArrowArrayData>` (~1 us), then `array_to_rust` re-imports via `make_array()` (~1 us). By returning an opaque `NativeArrowArray` handle (wrapping `Arc<dyn Array>`) that stays on the Rust side, we eliminate both the export and import. Additionally, `catch_and_log_exceptions` on `log()` adds ~0.3 us per call that can be avoided on the hot path.

## Code changes

### Rust (`rerun_py/src/arrow.rs`)
- **`NativeArrowArray` pyclass**: frozen struct wrapping `Arc<dyn Array>` with `__len__()` and `to_pyarrow()` methods
- **`build_fixed_size_list_array`**: changed return type from `PyArrowType<ArrowArrayData>` to `NativeArrowArray` — eliminates `.to_data()` export
- **`array_to_rust`**: added fast-path downcast for `NativeArrowArray` — just clones the `Arc` instead of going through PyArrow FFI import

### Rust (`rerun_py/src/python_bridge.rs`)
- Registered `NativeArrowArray` class in the `rerun_bindings` module

### Python (`rerun_py/rerun_sdk/rerun/_baseclasses.py`)
- `BaseBatch.__init__`: when `_native_to_pa_array` returns a `NativeArrowArray`, stores it in `_native_array` without setting `pa_array` (deferred)
- `BaseBatch.__getattr__`: lazily converts `NativeArrowArray → pa.Array` via `.to_pyarrow()` on first `.pa_array` access — zero cost on the hot path, cold-path consumers see a real `pa.Array`
- `BaseBatch.__len__`: reads from `_native_array` directly when available
- `BaseBatch.__eq__`: uses `as_arrow_array()` to ensure proper pyarrow comparison

### Python (`rerun_py/rerun_sdk/rerun/_log.py`)
- `log()`: replaced `@catch_and_log_exceptions()` decorator with inline `try/except`. Hot path skips the decorator entirely; errors fall through to `_log_with_catch()` which has the full error handling.
- `_log_components()`: hot path reads `batch._native_array` directly via `getattr()`, keeping NativeArrowArray on the Rust side. Falls back to `batch.as_arrow_array()` for non-native batches.

## Results (release build)

### Micro-benchmarks

| Metric | Before (H9) | After (H10) | Change |
|--------|-------------|-------------|--------|
| `rr.log(path, Transform3D(...))` | ~12.0 us | ~5.98 us | **-50%** |
| `rr.log(path, pre-built transform)` | ~3.3 us | ~1.75 us | **-47%** |
| `_log_components(path, batches)` | ~3.0 us | ~0.86 us | **-71%** |
| `bindings.log_arrow_msg` (NativeArrowArray) | ~2.6 us | ~0.64 us | **-75%** |
| `Translation3DBatch._native_to_pa_array(np)` | ~1.7 us | ~0.50 us | **-71%** |

### Integration benchmark

| Metric | Before (H9) | After (H10) | Change |
|--------|-------------|-------------|--------|
| Full log (construct + log) | 73.0k xforms/s | **135k xforms/s** | **+85%** |

### Key component timings (Stage 11)

| Operation | Time |
|-----------|------|
| `build_fixed_size_list_array(np, 3)` | 0.20 us |
| `build_fixed_size_list_array(np, 9)` | 0.21 us |
| `NativeArrowArray.to_pyarrow()` | 1.01 us |
| `len(NativeArrowArray)` | 0.03 us |
| `hasattr(transform, 'as_component_batches')` | 0.03 us |

## Architecture

```
HOT PATH (log):
  numpy → Rust build_fixed_size_list_array()
    → copy bytes → Arc<dyn Array> → NativeArrowArray handle
    → stored in BaseBatch._native_array (no PyArrow export)
    → _log_components reads _native_array directly → dict → bindings.log_arrow_msg
    → Rust array_to_rust() → downcast NativeArrowArray → clone Arc (no PyArrow import)

COLD PATH (compound types, __str__, send_columns, tests):
  Access .pa_array → triggers __getattr__ → .to_pyarrow() → caches pa.Array
  Subsequent .pa_array access → returns cached pa.Array directly
```

## Decision: KEEP
Eliminates ~6 us from the full pipeline (50% reduction). The NativeArrowArray is a clean abstraction — 15 lines of Rust, lazy conversion preserves full backwards compatibility for `.pa_array` and `.as_arrow_array()`.
