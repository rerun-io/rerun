use itertools::izip;

/// Are two arrays equal, ignoring small numeric differences?
pub fn approximate_equals(left: &arrow::array::ArrayData, right: &arrow::array::ArrayData) -> bool {
    if left.data_type() != right.data_type()
        || left.len() != right.len()
        || left.offset() != right.offset()
    {
        return false;
    }

    let data_type = left.data_type();

    if left.null_count() != right.null_count() {
        return false;
    }
    if left.nulls() != right.nulls() {
        return false;
    }

    {
        // Compare buffers:
        let left_buffers = left.buffers();
        let right_buffers = right.buffers();

        if left_buffers.len() != right_buffers.len() {
            return false;
        }

        for (i, (left_buff, right_buff)) in izip!(left_buffers, right_buffers).enumerate() {
            if left_buff.len() != right_buff.len() {
                return false;
            }

            if data_type == &arrow::datatypes::DataType::Float32 {
                // Approximate compare to accommodate differences in snippet output from Python/C++/Rust
                let left_floats = left_buff.typed_data::<f32>();
                let right_floats = right_buff.typed_data::<f32>();
                for (&l, &r) in izip!(left_floats, right_floats) {
                    if !almost_equal_f32(l, r, 1e-5) {
                        re_log::debug!("Significant f32 difference: {l} vs {r}");
                        return false;
                    }
                }
            } else if data_type == &arrow::datatypes::DataType::Float64 {
                // Approximate compare to accommodate differences in snippet output from Python/C++/Rust
                let left_floats = left_buff.typed_data::<f64>();
                let right_floats = right_buff.typed_data::<f64>();
                for (&l, &r) in izip!(left_floats, right_floats) {
                    if !almost_equal_f32(l as f32, r as f32, 1e-5) {
                        re_log::debug!("Significant f64 difference: {l} vs {r}");
                        return false;
                    }
                }
            } else if left_buff != right_buff {
                re_log::debug!("Difference in buffer {i} of array of datatype {data_type:?}");
                return false;
            }
        }
    }

    {
        // Compare children:
        let left_children = left.child_data();
        let right_children = right.child_data();

        if left_children.len() != right_children.len() {
            return false;
        }

        for (left_child, right_child) in izip!(left_children, right_children) {
            if !approximate_equals(left_child, right_child) {
                return false;
            }
        }
    }

    true
}

/// Return true when arguments are the same within some rounding error.
///
/// For instance `almost_equal(x, x.to_degrees().to_radians(), f32::EPSILON)` should hold true for all x.
/// The `epsilon`  can be `f32::EPSILON` to handle simple transforms (like degrees -> radians)
/// but should be higher to handle more complex transformations.
pub fn almost_equal_f32(a: f32, b: f32, epsilon: f32) -> bool {
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
        0.0_f32,
        f32::MIN_POSITIVE,
        1e-20,
        1e-10,
        f32::EPSILON,
        0.1,
        0.99,
        1.0,
        1.001,
        1e10,
        f32::MAX / 100.0,
        // f32::MAX, // overflows in rad<->deg test
        f32::INFINITY,
    ] {
        for &x in &[-x, x] {
            for roundtrip in &[
                |x: f32| x.to_degrees().to_radians(),
                |x: f32| x.to_radians().to_degrees(),
            ] {
                let epsilon = f32::EPSILON;
                assert!(
                    almost_equal_f32(x, roundtrip(x), epsilon),
                    "{} vs {}",
                    x,
                    roundtrip(x)
                );
            }
        }
    }
}
