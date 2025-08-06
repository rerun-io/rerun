use anyhow::{Context as _, bail, ensure};
use half::f16;
use itertools::izip;

use crate::format_data_type;

/// Are two arrays equal, ignoring small numeric differences?
///
/// Returns `Ok` if similar.
/// If there is a difference, a description of that difference is returned in `Err`.
/// We use [`anyhow`] to provide context.
pub fn ensure_similar(
    left: &arrow::array::ArrayData,
    right: &arrow::array::ArrayData,
) -> anyhow::Result<()> {
    ensure!(left.data_type() == right.data_type());

    let data_type = left.data_type();

    if matches!(data_type, arrow::datatypes::DataType::Union { .. }) {
        // We encode arrow unions slightly different in Python and Rust.
        // TODO(#6388): Remove this hack once we have stopped using arrow unions.
        ensure!(left == right);
        return Ok(());
    }

    ensure!(left.len() == right.len());
    ensure!(left.offset() == right.offset());
    ensure!(left.null_count() == right.null_count());
    ensure!(left.nulls() == right.nulls());

    {
        // Compare buffers:
        let left_buffers = left.buffers();
        let right_buffers = right.buffers();

        ensure!(left_buffers.len() == right_buffers.len());

        for (i, (left_buff, right_buff)) in izip!(left_buffers, right_buffers).enumerate() {
            ensure_buffers_equal(left_buff, right_buff, data_type)
                .with_context(|| format!("Datatype {}", format_data_type(data_type)))
                .with_context(|| format!("Buffer {i}"))?;
        }
    }

    {
        // Compare children:
        let left_children = left.child_data();
        let right_children = right.child_data();

        ensure!(left_children.len() == right_children.len());

        for (i, (left_child, right_child)) in izip!(left_children, right_children).enumerate() {
            ensure_similar(left_child, right_child)
                .with_context(|| format!("Datatype {}", format_data_type(data_type)))
                .with_context(|| format!("Child {i}"))?;
        }
    }

    Ok(())
}

fn ensure_buffers_equal(
    left_buff: &arrow::buffer::Buffer,
    right_buff: &arrow::buffer::Buffer,
    data_type: &arrow::datatypes::DataType,
) -> anyhow::Result<()> {
    ensure!(left_buff.len() == right_buff.len());

    if data_type == &arrow::datatypes::DataType::Float16 {
        // Approximate compare to accommodate differences in snippet output from Python/C++/Rust
        let left_floats = left_buff.typed_data::<f16>();
        let right_floats = right_buff.typed_data::<f16>();
        for (&l, &r) in izip!(left_floats, right_floats) {
            if !almost_equal_f64(l.to_f64(), r.to_f64(), 1e-3) {
                bail!("Significant f16 difference: {l} vs {r}");
            }
        }
    } else if data_type == &arrow::datatypes::DataType::Float32 {
        // Approximate compare to accommodate differences in snippet output from Python/C++/Rust
        let left_floats = left_buff.typed_data::<f32>();
        let right_floats = right_buff.typed_data::<f32>();
        for (&l, &r) in izip!(left_floats, right_floats) {
            if !almost_equal_f64(l as f64, r as f64, 1e-3) {
                bail!("Significant f32 difference: {l} vs {r}");
            }
        }
    } else if data_type == &arrow::datatypes::DataType::Float64 {
        // Approximate compare to accommodate differences in snippet output from Python/C++/Rust
        let left_floats = left_buff.typed_data::<f64>();
        let right_floats = right_buff.typed_data::<f64>();
        for (&l, &r) in izip!(left_floats, right_floats) {
            if !almost_equal_f64(l, r, 1e-8) {
                bail!("Significant f64 difference: {l} vs {r}");
            }
        }
    } else {
        ensure!(left_buff == right_buff);
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
