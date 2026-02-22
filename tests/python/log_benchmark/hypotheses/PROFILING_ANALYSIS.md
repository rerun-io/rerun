# Detailed Profiling Analysis

## Full pipeline: `rr.log("path", rr.Transform3D(translation=list, mat3x3=np))` = 17.47 us

### Time Budget Breakdown

```
Full rr.log() call:                                17.47 us  (100%)
├── Transform3D construction:                       5.51 us  (31.5%)
│   ├── __attrs_init__ (8 converter calls):         5.15 us  (29.5%)  ← THIS IS THE BOTTLENECK
│   │   ├── Translation3DBatch(list):               1.90 us  (10.9%)
│   │   │   ├── catch_and_log_exceptions:           0.29 us
│   │   │   ├── _native_to_pa_array(list):          1.70 us
│   │   │   │   ├── flat_np_float32_array(list):    0.30 us
│   │   │   │   ├── np.ascontiguousarray:           ~0.10 us
│   │   │   │   └── pa.FixedSizeListArray:          ~1.02 us  ← PyArrow overhead
│   │   │   └── isinstance + other overhead:        ~0.20 us
│   │   ├── TransformMat3x3Batch(np):               2.32 us  (13.3%)
│   │   │   ├── catch_and_log_exceptions:           0.29 us
│   │   │   ├── _native_to_pa_array(np):            ~1.41 us
│   │   │   │   ├── numpy ops (reshape+T+ravel):    ~0.40 us
│   │   │   │   └── pa.FixedSizeListArray:          ~1.04 us  ← PyArrow overhead
│   │   │   └── isinstance + other overhead:        ~0.30 us
│   │   └── 6x _converter(None):                    0.19 us   (negligible)
│   └── catch_and_log_exceptions (init):            0.29 us
│
├── rr.log() overhead:                              4.83 us  (27.6%)
│   ├── catch_and_log_exceptions (log decorator):   0.29 us
│   ├── as_component_batches():                     0.51 us
│   ├── _log_components Python overhead:            0.90 us
│   │   ├── build instanced dict:                   0.20 us
│   │   ├── extract descriptors:                    0.11 us
│   │   ├── extract arrow arrays:                   0.15 us
│   │   └── zip/set logic:                          ~0.44 us
│   └── bindings.log_arrow_msg (Rust FFI):          2.50 us  (14.3%)
│       ├── descriptor_to_rust (x2):                ~0.10 us  (fast with H1 downcast)
│       ├── array_to_rust / Arrow C Data (x2):      ~1.00 us  ← Arrow transfer
│       └── PendingRow + chunk store:               ~1.40 us  ← Rust-side work
│
└── set_time (x2, in full benchmark):               0.52 us  (3.0%)
    └── 2x rr.set_time():                           0.26 us each
```

### Key Findings

1. **pa.FixedSizeListArray.from_arrays() costs ~1.0 us per call** — 2 calls = 2.0 us (11.4% of total).
   For 3 floats (Vec3D) and 9 floats (Mat3x3), this is enormous overhead per datum.

2. **catch_and_log_exceptions costs 0.29 us per entry** — 4 entries = 1.16 us (6.6% of total).
   Compare to try/finally at 0.03 us — it's 10x slower even after __dict__ optimization.

3. **bindings.log_arrow_msg costs 2.50 us** — 14.3% of total, pure Rust.
   About 1.0 us is Arrow C Data Interface transfer, 1.4 us is Rust-side work.

4. **_log_components Python overhead is only 0.90 us** — already lean.

5. **The 6 unused _converter(None) calls are only 0.19 us** — negligible.

6. **__attrs_init__ itself (with all converters) is 5.15 us** — 29.5% of total.
   But most of this is the 2 actual batch constructions (4.22 us).
   The attrs machinery overhead (calling converters, setting fields) adds ~0.74 us.

### Largest Remaining Opportunities (ordered by expected impact)

1. **Bypass PyArrow for small arrays** (~2.0 us = 11.4%):
   `pa.FixedSizeListArray.from_arrays()` has ~1 us overhead per call regardless of array size.
   Building Arrow arrays in Rust from numpy buffers would eliminate this.

2. **Reduce catch_and_log_exceptions to near-zero** (~1.16 us = 6.6%):
   Even with __dict__ optimization, 0.29 us * 4 calls = 1.16 us. A try/finally would be 0.12 us.
   Could make the context manager nearly free by deferring all work to __exit__ on exception only.

3. **Reduce Rust-side log_arrow_msg overhead** (~2.50 us = 14.3%):
   The Arrow C Data Interface transfer and Rust-side row building could potentially be optimized.
   A specialized Rust endpoint for common archetypes could bypass generic Arrow serialization.

4. **Skip _log_components Python dict-building** (~0.90 us = 5.2%):
   Pass batches directly to Rust instead of building a Python dict first.

5. **Eliminate attrs __attrs_init__ overhead** (~0.74 us = 4.2%):
   Direct field assignment instead of going through attrs converters.
