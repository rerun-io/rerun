use half::f16;
use itertools::izip;

use crate::ArrayComparisonError;

/// Are two arrays equal, ignoring small numeric differences?
///
/// Returns `Ok` if similar.
/// If there is a difference, a description of that difference is returned in `Err`.
pub fn ensure_similar(
    left: &arrow::array::ArrayData,
    right: &arrow::array::ArrayData,
) -> Result<(), ArrayComparisonError> {
    if left.data_type() != right.data_type() {
        return Err(ArrayComparisonError(format!(
            "data type mismatch: {} != {}",
            left.data_type(),
            right.data_type()
        )));
    }

    let data_type = left.data_type();

    if matches!(data_type, arrow::datatypes::DataType::Union { .. }) {
        // We encode arrow unions slightly different in Python and Rust.
        // TODO(#6388): Remove this hack once we have stopped using arrow unions.
        if left != right {
            return Err(ArrayComparisonError("union arrays differ".into()));
        }
        return Ok(());
    }

    if left.len() != right.len() {
        return Err(ArrayComparisonError(format!(
            "length mismatch: {} != {}",
            left.len(),
            right.len()
        )));
    }
    if left.offset() != right.offset() {
        return Err(ArrayComparisonError(format!(
            "offset mismatch: {} != {}",
            left.offset(),
            right.offset()
        )));
    }
    if left.null_count() != right.null_count() {
        return Err(ArrayComparisonError(format!(
            "null count mismatch: {} != {}",
            left.null_count(),
            right.null_count()
        )));
    }
    if left.nulls() != right.nulls() {
        return Err(ArrayComparisonError("null bitmaps differ".into()));
    }

    {
        // Compare buffers:
        let left_buffers = left.buffers();
        let right_buffers = right.buffers();

        if left_buffers.len() != right_buffers.len() {
            return Err(ArrayComparisonError(format!(
                "buffer count mismatch: {} != {}",
                left_buffers.len(),
                right_buffers.len()
            )));
        }

        for (i, (left_buff, right_buff)) in izip!(left_buffers, right_buffers).enumerate() {
            ensure_buffers_equal(left_buff, right_buff, data_type).map_err(|e| {
                ArrayComparisonError(format!("Buffer {i} (Datatype {data_type}): {e}"))
            })?;
        }
    }

    {
        // Compare children:
        let left_children = left.child_data();
        let right_children = right.child_data();

        if left_children.len() != right_children.len() {
            return Err(ArrayComparisonError(format!(
                "child count mismatch: {} != {}",
                left_children.len(),
                right_children.len()
            )));
        }

        for (i, (left_child, right_child)) in izip!(left_children, right_children).enumerate() {
            ensure_similar(left_child, right_child).map_err(|e| {
                ArrayComparisonError(format!("Child {i} (Datatype {data_type}): {e}"))
            })?;
        }
    }

    Ok(())
}

fn ensure_buffers_equal(
    left_buff: &arrow::buffer::Buffer,
    right_buff: &arrow::buffer::Buffer,
    data_type: &arrow::datatypes::DataType,
) -> Result<(), ArrayComparisonError> {
    if left_buff.len() != right_buff.len() {
        return Err(ArrayComparisonError(format!(
            "buffer length mismatch: {} != {}",
            left_buff.len(),
            right_buff.len()
        )));
    }

    if data_type == &arrow::datatypes::DataType::Float16 {
        // Approximate compare to accommodate differences in snippet output from Python/C++/Rust
        let left_floats = left_buff.typed_data::<f16>();
        let right_floats = right_buff.typed_data::<f16>();
        for (&l, &r) in izip!(left_floats, right_floats) {
            if !almost_equal_f64(l.to_f64(), r.to_f64(), 1e-3) {
                return Err(ArrayComparisonError(format!(
                    "Significant f16 difference: {l} vs {r}"
                )));
            }
        }
    } else if data_type == &arrow::datatypes::DataType::Float32 {
        // Approximate compare to accommodate differences in snippet output from Python/C++/Rust
        let left_floats = left_buff.typed_data::<f32>();
        let right_floats = right_buff.typed_data::<f32>();
        for (&l, &r) in izip!(left_floats, right_floats) {
            if !almost_equal_f64(l as f64, r as f64, 1e-3) {
                return Err(ArrayComparisonError(format!(
                    "Significant f32 difference: {l} vs {r}"
                )));
            }
        }
    } else if data_type == &arrow::datatypes::DataType::Float64 {
        // Approximate compare to accommodate differences in snippet output from Python/C++/Rust
        let left_floats = left_buff.typed_data::<f64>();
        let right_floats = right_buff.typed_data::<f64>();
        for (&l, &r) in izip!(left_floats, right_floats) {
            if !almost_equal_f64(l, r, 1e-8) {
                return Err(ArrayComparisonError(format!(
                    "Significant f64 difference: {l} vs {r}"
                )));
            }
        }
    } else if left_buff != right_buff {
        return Err(ArrayComparisonError("buffer contents differ".into()));
    }

    Ok(())
}

/// Return true when arguments are the same within some rounding error.
///
/// For instance `almost_equal(x, x.to_degrees().to_radians(), f64::EPSILON)` should hold true for all x.
/// The `epsilon`  can be `f64::EPSILON` to handle simple transforms (like degrees -> radians)
/// but should be higher to handle more complex transformations.
pub fn almost_equal_f64(a: f64, b: f64, epsilon: f64) -> bool {
    if a == b {
        true // handle infinites
    } else {
        let abs_max = a.abs().max(b.abs());
        abs_max <= epsilon || ((a - b).abs() / abs_max) <= epsilon
    }
}

#[test]
fn test_almost_equal() {
    for &x in &[
        0.0_f64,
        f64::MIN_POSITIVE,
        1e-20,
        1e-10,
        f64::EPSILON,
        0.1,
        0.99,
        1.0,
        1.001,
        1e10,
        f64::MAX / 100.0,
        // f64::MAX, // overflows in rad<->deg test
        f64::INFINITY,
    ] {
        for &x in &[-x, x] {
            for roundtrip in &[
                |x: f64| x.to_degrees().to_radians(),
                |x: f64| x.to_radians().to_degrees(),
            ] {
                let epsilon = f64::EPSILON;
                assert!(
                    almost_equal_f64(x, roundtrip(x), epsilon),
                    "{} vs {}",
                    x,
                    roundtrip(x)
                );
            }
        }
    }
}
