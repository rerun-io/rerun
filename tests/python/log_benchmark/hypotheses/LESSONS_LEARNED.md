# Lessons Learned: Python Logging Performance Investigation

## Investigation Overview

**Goal**: Make `rr.log("path", rr.Transform3D(translation=list, mat3x3=np))` as fast as possible.

**Result**: 36.8k → 135k transforms/s (**3.7x improvement**), from ~27 us to ~6 us per call.

**Approach**: Bottom-up micro-benchmarking. Isolated each stage of the pipeline, measured independently, identified the top time sinks, and attacked them in order of expected impact.

## Methodology That Worked

### 1. The micro-benchmark harness was essential

`tests/python/log_benchmark/micro_benchmark.py` was the single most valuable tool. It isolates every stage — construction, arrow conversion, component batching, dict building, Rust FFI — so you can see exactly where time goes and whether a change actually helps.

Key pattern: always benchmark both the **isolated operation** and the **full pipeline**. Isolated benchmarks show theoretical savings; full pipeline shows actual impact (which is often less due to cache effects and Python overhead that doesn't show up in isolation).

### 2. Profile-driven, not guess-driven

The initial PROFILING_ANALYSIS.md time budget identified the actual bottlenecks. Several intuitive "optimizations" would have been wasted:
- The 6 unused `_converter(None)` calls look wasteful but cost only 0.19 us total — not worth touching
- `isinstance(data, pa.Array)` looks like it could be slow (Protocol check) but costs 0.03 us — irrelevant
- The `set_time` calls are only 0.25 us each — not a bottleneck

### 3. Measure, change, re-measure, repeat

Every hypothesis was measured before and after on the same machine with the same build. The cumulative tracking in RESULTS.md caught cases where individual improvements overlapped or didn't compound as expected.

## Key Technical Insights

### Insight 1: PyArrow is expensive for small arrays

`pa.FixedSizeListArray.from_arrays()` has ~1.0 us **fixed overhead** regardless of array size. For logging a single Vec3D (3 floats = 12 bytes), this overhead dominates the actual data handling. The solution (H7) was to build the Arrow array in Rust directly from the numpy buffer, which costs only ~0.2 us.

**Lesson**: PyArrow is designed for analytical workloads with large arrays. Per-row logging of small fixed-size types pays an outsized tax.

### Insight 2: The PyArrow FFI round-trip is the biggest single cost

The Arrow C Data Interface export (`.to_data()`) + import (`make_array()`) costs ~1 us per array. With 2 components per Transform3D, that's ~2 us just for data that's already in the right format on the Rust side.

The NativeArrowArray pattern (H10) was the largest single win because it eliminates this entirely: data stays as `Arc<dyn Array>` on the Rust side and the Python handle is just a pointer. When it comes back to Rust, we clone the Arc (~0.05 us) instead of going through FFI (~1 us).

**Lesson**: When you have a Rust extension module and the data originates in Rust, keeping it opaque on the Rust side and only materializing it in Python when cold-path consumers need it can be a huge win. The `__getattr__` lazy conversion pattern is a clean way to maintain backwards compatibility.

### Insight 3: Python context managers have non-trivial cost

`catch_and_log_exceptions` as a context manager costs 0.29 us per enter/exit — about 10x more than a bare `try/except` (0.03 us). With 4 nested uses on the hot path, that's >1 us just for error handling infrastructure.

**Lesson**: For inner-loop code, prefer inline `try/except` over context managers. Reserve context managers for readability in cold paths. The decorator on `log()` was replaced with a fast try/except that falls back to the full error handler only on exception.

### Insight 4: `threading.local()` access patterns matter

The original code used `getattr(threading_local, "field", default)` which goes through Python's full attribute lookup. Switching to `threading_local.__dict__.get("field")` saved ~10% overall (H2). This is because `__dict__` is a C-level dict access rather than going through `__getattribute__`.

### Insight 5: attrs converters are cheap but the machinery adds up

Individual `_converter(None)` calls cost only 0.05 us, but `__attrs_init__` with 8 fields (including converter dispatch, field validation, and slot assignment) adds ~0.7 us. For the hot path, this is a significant fraction of the remaining budget.

### Insight 6: Caching works because archetypes are repetitive

`ComponentDescriptor` objects and archetype field name lists are identical across every call to the same archetype. Caching them (H3+H5) on the class eliminates repeated string concatenation and `fields()` introspection.

## Architecture of the Current Hot Path

```
User calls: rr.log("path", rr.Transform3D(translation=[1,2,3], mat3x3=np.eye(3)))

1. Transform3D.__init__()                                         ~2.46 us
   ├─ __attrs_init__ dispatches to converters
   ├─ Translation3DBatch.__init__()                               ~0.73 us
   │  ├─ _native_to_pa_array → Vec3DExt.native_to_pa_array_override
   │  │  ├─ flat_np_float32_array_from_array_like(list, 3)        ~0.31 us
   │  │  └─ rerun_bindings.build_fixed_size_list_array(np, 3)     ~0.20 us
   │  │     → Returns NativeArrowArray (opaque Rust handle)
   │  └─ Stored in self._native_array (pa_array NOT set yet)
   ├─ TransformMat3x3Batch.__init__()                             ~1.03 us
   │  └─ Same pattern: numpy prep → build_fixed_size_list_array → NativeArrowArray
   └─ 6x _converter(None) → returns None immediately              ~0.20 us

2. rr.log()                                                       ~1.75 us
   ├─ Inlined try/except (no catch_and_log_exceptions overhead)
   ├─ hasattr(entity, 'as_component_batches')                     ~0.03 us
   ├─ list(entity.as_component_batches())                         ~0.54 us
   │  ├─ Iterates 2 non-None component fields (cached field names)
   │  ├─ Looks up cached ComponentDescriptor per field
   │  └─ Returns [DescribedComponentBatch, DescribedComponentBatch]
   └─ _log_components()                                           ~0.86 us
      ├─ For each batch: getattr(batch, '_native_array', None)
      │  → Gets NativeArrowArray directly (no PyArrow conversion!)
      ├─ Builds dict: {descriptor: NativeArrowArray, ...}
      └─ bindings.log_arrow_msg()                                 ~0.62 us
         ├─ descriptor_to_rust: downcast PyComponentDescriptor     ~0.05 us each
         ├─ array_to_rust: downcast NativeArrowArray → clone Arc   ~0.05 us each
         └─ PendingRow creation + recording stream push            ~0.40 us

COLD PATH (only when .pa_array is accessed, e.g. __str__, compound types):
   BaseBatch.__getattr__('pa_array')
   → self._native_array.to_pyarrow()                              ~1.01 us
   → Caches result in self.pa_array (subsequent access is free)
```

## Where the Remaining ~6 us Goes

```
Transform3D construction:     2.46 us  (41%)  ← Main bottleneck
  attrs machinery:            0.70 us  (12%)  -- converters, field dispatch, __attrs_init__
  numpy ops:                  0.76 us  (13%)  -- asarray, reshape, transpose, ravel, ascontiguous
  Rust arrow builder:         0.41 us   (7%)  -- build_fixed_size_list_array x2
  BaseBatch overhead:         0.59 us  (10%)  -- isinstance check, hasattr, _native_array storage

rr.log() glue:                1.75 us  (29%)
  as_component_batches:       0.54 us   (9%)  -- getattr x8, None checks, list building
  bindings.log_arrow_msg:     0.62 us  (10%)  -- Rust-side row creation + recording push
  Python overhead:            0.59 us  (10%)  -- try/except, hasattr, list(), dict building

Unaccounted:                  1.77 us  (30%)  -- Python interpreter overhead, function call
                                                 dispatch, memory allocation, lambda closures
                                                 in benchmark vs real overhead
```

## Concrete Next Steps for Another 50% Improvement

### H11: Move batch construction to Rust (~1.5 us savings, ~25% improvement)

The biggest remaining target. Currently each batch does:
1. Python: `flat_np_float32_array_from_array_like(data, dim)` — validates/converts input to flat f32 numpy
2. Python: `np.ascontiguousarray(...)` — ensures contiguous layout
3. Python→Rust: `build_fixed_size_list_array(flat, size)` — builds Arrow array

All three steps could be a single Rust `#[pyfunction]` that accepts arbitrary Python input (list, numpy array, tuple) and returns a NativeArrowArray. This eliminates:
- The `flat_np_float32_array_from_array_like` Python function (0.31 us for lists, 0.19 us for numpy)
- The `np.ascontiguousarray` call
- One Python→Rust FFI crossing

For Mat3x3, the numpy reshape/transpose/ravel (0.45 us) could also move to Rust.

**Estimated savings**: ~0.8 us (Translation3D) + ~0.7 us (Mat3x3) = ~1.5 us

### H12: Bypass attrs for hot-path archetypes (~0.7 us savings, ~12% improvement)

`__attrs_init__` with 8 fields costs ~0.7 us in overhead beyond the actual converter work. For the common case of `Transform3D(translation=X, mat3x3=Y)` where only 2 of 8 fields are set, we're paying for 6 unnecessary converter dispatch + field assignment cycles.

Options:
- **A**: Generate a fast `__init__` that checks common signatures and short-circuits attrs entirely
- **B**: A Rust `#[pyfunction]` `build_transform3d(translation=..., mat3x3=...)` that constructs the batches and returns the component dict directly, bypassing Python archetype construction

Option B is more radical but could eliminate the entire construction step (~2.46 us → ~0.5 us).

### H13: Fuse as_component_batches + _log_components (~0.5 us savings, ~8% improvement)

Currently `as_component_batches()` builds a `list[DescribedComponentBatch]` that `_log_components` immediately iterates to build a dict. This intermediate list allocation + iteration could be eliminated by having `_log_components` directly iterate the archetype's fields.

A Rust `#[pyfunction]` could accept the archetype object directly, call `getattr` for each known field, and build the row without any Python intermediate structures.

### H14: Pool/reuse RowId generation (~0.1-0.2 us savings)

`RowId::new()` involves timestamp + counter. If the recording stream could pre-allocate or batch RowIds, this small per-call cost could be amortized.

### H15: Specialize log() for common cases (~0.3 us savings)

The current `log()` function handles `*extra` args, `Iterable` inputs, and list entity paths. For the common case of `log(str, AsComponents)`, a specialized fast path could skip all the type checking and go straight to `_log_components`.

### Combined estimate

| Hypothesis | Savings | Cumulative |
|-----------|---------|------------|
| Current | — | 5.98 us |
| H11: Batch construction in Rust | ~1.5 us | ~4.5 us |
| H12: Bypass attrs | ~0.7 us | ~3.8 us |
| H13: Fuse batches+log | ~0.5 us | ~3.3 us |
| H15: Specialize log() | ~0.3 us | ~3.0 us |

This would put us at **~3.0 us/call → ~330k transforms/s**, another ~2.5x from current.

## Files and Tools

### Key files
- `tests/python/log_benchmark/micro_benchmark.py` — the micro-benchmark harness
- `tests/python/log_benchmark/test_log_benchmark.py` — the integration benchmark (use `transform3d --num-entities 100 --num-time-steps 1000`)
- `tests/python/log_benchmark/hypotheses/` — all hypothesis write-ups and results
- `rerun_py/src/arrow.rs` — NativeArrowArray, build_fixed_size_list_array, array_to_rust
- `rerun_py/rerun_sdk/rerun/_baseclasses.py` — BaseBatch with lazy NativeArrowArray conversion
- `rerun_py/rerun_sdk/rerun/_log.py` — log() and _log_components hot path
- `rerun_py/rerun_sdk/rerun/datatypes/vec3d_ext.py` — Vec3D arrow construction
- `rerun_py/rerun_sdk/rerun/datatypes/mat3x3_ext.py` — Mat3x3 arrow construction
- `rerun_py/rerun_sdk/rerun/archetypes/transform3d_ext.py` — Transform3D __init__ with inlined error handling

### Commands
```bash
pixi run py-build-release                    # Build (always use release for benchmarking!)
pixi run uvpy tests/python/log_benchmark/micro_benchmark.py   # Micro-benchmarks
pixi run uvpy -m tests.python.log_benchmark.test_log_benchmark transform3d --num-entities 100 --num-time-steps 1000  # Integration
pixi run uvpy -m pytest -x rerun_py/tests/   # Verify nothing broke
pixi run py-lint                              # Type checking
cargo clippy -p rerun_py --no-default-features  # Rust linting
```

### Patterns established
- **NativeArrowArray + __getattr__ lazy conversion**: opaque Rust handle on hot path, transparent pyarrow on cold path. Re-use this pattern for any future Rust-built arrays.
- **Inline try/except on hot path**: faster than context managers. Fall back to full error handler via a `_with_catch` wrapper function.
- **Class-level caching**: `cls.__dict__.get()` + write-back for per-archetype metadata.
- **getattr fast path in _log_components**: `getattr(batch, '_native_array', None)` to bypass as_arrow_array().

## What NOT to Optimize

- **_converter(None)** — 0.05 us each, 6 calls = 0.30 us. Not worth the complexity of conditional dispatch.
- **isinstance checks** — 0.03 us. Negligible.
- **set_time** — 0.25 us per call, outside the log() pipeline. Already fast.
- **ComponentDescriptor construction** — 0.11 us, already cached. Can't improve further without removing the abstraction.
- **DescribedComponentBatch allocation** — 0.08 us per wrapper. Would need to eliminate the object entirely.
