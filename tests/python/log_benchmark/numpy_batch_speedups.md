# Python Logging Speedups: Numpy-Native Batch Storage

Branch `nick/log_nparray` vs `main` (baseline).

## Summary

By storing numpy arrays directly in component batches (instead of converting to PyArrow),
and only building Arrow arrays in Rust at log time, we achieve **2.5-3.2x end-to-end speedup**
for `rr.log()` calls and **4-5.5x speedup** for individual batch construction.

Key changes:
- `_ext.py` files return flat numpy arrays instead of `pa.FixedSizeListArray`
- `BaseBatch` stores numpy in `_numpy_data` with lazy `pa_array` conversion
- `_log_components` passes `(numpy_array, list_size)` tuples to Rust
- Rust `numpy_to_fixed_size_list()` builds Arrow directly from numpy buffers
- Descriptor caching on `Archetype` class
- Inline try/except replacing context manager on hot paths

## Full `rr.log()` pipeline

| Benchmark | main | optimized | Speedup |
|---|---|---|---|
| `rr.log(Transform3D(trans+mat3x3))` | 22.56 us | 6.96 us | **3.2x** |
| `rr.log(Transform3D(trans+quat))` | 18.98 us | 6.76 us | **2.8x** |
| `rr.log(Arrows3D(2 arrows))` | 19.05 us | 6.50 us | **2.9x** |
| `rr.log(Points3D(2 points))` | 13.57 us | 4.67 us | **2.9x** |
| `rr.log(Points2D(2 points))` | 13.67 us | 4.79 us | **2.9x** |
| `rr.log(pre-built transform)` | 7.24 us | 2.87 us | **2.5x** |
| `rr.log(Pinhole, pre-built)` | 7.38 us | 2.95 us | **2.5x** |

## Batch construction per datatype

| Datatype | main | optimized | Speedup |
|---|---|---|---|
| Position3DBatch(np) | 2.06 us | 0.38 us | **5.4x** |
| Position2DBatch(np) | 2.01 us | 0.36 us | **5.6x** |
| Vec2DBatch(list) | 2.15 us | 0.48 us | **4.5x** |
| Vec3DBatch(list) | 2.22 us | 0.52 us | **4.3x** |
| QuaternionBatch(list) | 2.24 us | 0.51 us | **4.4x** |
| Mat3x3Batch(np.eye(3)) | 3.07 us | 0.75 us | **4.1x** |
| Mat4x4Batch(np.eye(4)) | 3.47 us | 1.67 us | **2.1x** |

## Primitive scalar batch construction

Eliminates `pa.array()` for primitive types by returning numpy directly from
`_native_to_pa_array` and building Arrow arrays in Rust via `numpy_to_primitive_array()`.

| Benchmark | main | optimized | Speedup |
|---|---|---|---|
| `Float32Batch(1.0)` | 1.76 us | 0.28 us | **6.3x** |
| `Float32Batch(np 100)` | 1.60 us | 0.22 us | **7.3x** |
| `UInt16Batch(42)` | 1.72 us | 0.28 us | **6.1x** |
| `UInt32Batch(0xFF0000FF)` | 1.76 us | 0.28 us | **6.3x** |
| `BoolBatch(True)` | 1.69 us | 0.27 us | **6.3x** |
| `Float64Batch(1.0)` | 1.69 us | 0.28 us | **6.0x** |
| `RadiusBatch(1.0)` | 1.72 us | 0.29 us | **5.9x** |
| `ColorBatch(0xFF0000FF)` | 2.84 us | 0.74 us | **3.8x** |
| `ClassIdBatch(42)` | 1.74 us | 0.28 us | **6.2x** |
| `OpacityBatch(0.5)` | 1.73 us | 0.29 us | **6.0x** |
| `ShowLabelsBatch(True)` | 1.68 us | 0.28 us | **6.0x** |
| `ScalarBatch(1.0)` | 1.79 us | 0.31 us | **5.8x** |

## Scalar-heavy archetype end-to-end

Points3D with positions, colors, radii, and class_ids.

| Benchmark | main | optimized | Speedup |
|---|---|---|---|
| `rr.log(Points3D full, pre-built)` | 11.00 us | 4.61 us | **2.4x** |
| `rr.log(Points3D full, from scratch)` | 41.72 us | 17.20 us | **2.4x** |

## String and variable-length batch construction

Uses `(data_bytes, offsets, inner_size)` 3-tuples to pass variable-length data to Rust,
bypassing PyArrow's `ListArray.from_arrays`, `pa.concat_arrays`, and `pa.array(strings)`.
- Strings (`inner_size == -1`): UTF-8 bytes + offsets → Rust builds `StringArray`
- Blobs (`inner_size == 0`): flat uint8 + offsets → Rust builds `ListArray<UInt8>`
- LineStrips (`inner_size == 2|3`): flat float32 + offsets → Rust builds `ListArray<FixedSizeList>`

| Benchmark | main | optimized | Speedup |
|---|---|---|---|
| `Utf8Batch('hello')` | 1.91 us | 0.72 us | **2.7x** |
| `Utf8Batch(['a', 'b', 'c'])` | 2.14 us | 1.11 us | **1.9x** |
| `LineStrip3DBatch([3-point strip])` | 7.67 us | 2.24 us | **3.4x** |
| `LineStrip2DBatch([3-point strip])` | 7.43 us | 2.19 us | **3.4x** |
| `BlobBatch(np 1000 bytes)` | 4.78 us | 0.88 us | **5.4x** |
| `BlobBatch(b'hello')` | 4.57 us | 0.92 us | **5.0x** |

## Enum batch construction

Replaces `pa.array(list, type=data_type)` with `np.array(list, dtype=np.uint8)` in
generated enum code. The Python list comprehension (`[Enum.auto(v).value for v in data]`)
remains the dominant cost; this eliminates the PyArrow overhead on top of it.
Falls back to `pa.array()` if any values are `None` (nullable enums).

| Benchmark | main | optimized | Speedup |
|---|---|---|---|
| `AggregationPolicyBatch('Average')` | 2.27 us | 0.62 us | **3.7x** |
| `ColormapBatch('Viridis')` | 2.29 us | 0.62 us | **3.7x** |
| `FillModeBatch('Solid')` | 2.22 us | 0.62 us | **3.6x** |
| `MarkerShapeBatch('Circle')` | 2.30 us | 0.61 us | **3.8x** |

## Key internals

| Stage | main | optimized | Speedup |
|---|---|---|---|
| `as_component_batches()` | 1.57 us | 0.45 us | **3.5x** |
| `__attrs_init__` | 5.95 us | 1.91 us | **3.1x** |
| `BaseBatch.__init__` | 2.28 us | 0.52 us | **4.4x** |
| `_log_components` | 3.89 us | 1.87 us | **2.1x** |
