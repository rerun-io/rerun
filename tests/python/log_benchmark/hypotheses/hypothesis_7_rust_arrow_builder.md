# Hypothesis 7: Bypass PyArrow — build Arrow FixedSizeListArrays in Rust

## Hypothesis
`pa.FixedSizeListArray.from_arrays()` costs ~1.0 us per call regardless of array size. For Vec3D (3 floats) and Mat3x3 (9 floats), this is pure overhead — the actual data is already a flat numpy array. Building the Arrow array in Rust directly from the numpy buffer eliminates this per-call PyArrow overhead.

## Code Changes

### Rust (`rerun_py/src/arrow.rs`)
Added `build_fixed_size_list_array(flat_array, list_size)` function:
- Accepts a flat `numpy::PyReadonlyArray1<f32>` and a list size
- Builds a `FixedSizeListArray` using `arrow::array::FixedSizeListArray::new()`
- Returns it as `PyArrowType<ArrowArrayData>` so Python gets a `pa.Array` back
- Inner field uses `nullable=false` to match Rerun's type schema

### Rust (`rerun_py/src/python_bridge.rs`)
Registered `build_fixed_size_list_array` as a `#[pyfunction]` in the module.

### Python (`datatypes/vec3d_ext.py`)
Replaced `pa.FixedSizeListArray.from_arrays(points, type=data_type)` with `rerun_bindings.build_fixed_size_list_array(points, 3)`.

### Python (`datatypes/mat3x3_ext.py`)
Replaced `pa.FixedSizeListArray.from_arrays(np.ascontiguousarray(float_arrays), type=data_type)` with `rerun_bindings.build_fixed_size_list_array(np.ascontiguousarray(float_arrays), 9)`.

## Results (release build, micro-benchmark)

| Metric | Before | After | Change |
|--------|--------|-------|--------|
| Translation3DBatch._native_to_pa_array(list, type) | ~1.9 us | ~1.8 us | -5% |
| Translation3DBatch._native_to_pa_array(np, type) | ~1.8 us | ~1.7 us | -6% |
| Full rr.log(path, Transform3D(...)) | ~17.5 us | ~15.5 us | -11% |

The savings are ~1.0 us across 2 calls (Vec3D + Mat3x3), which is partially masked by other per-call overhead in the batch constructors.

## Decision: KEEP
Eliminates ~2.0 us of PyArrow per-call overhead. The Rust function is 30 lines, straightforward, and the numpy crate was already a dependency.
