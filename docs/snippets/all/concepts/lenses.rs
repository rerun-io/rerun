//! Use lenses to extract struct fields and reroute data to a different entity.

use std::sync::Arc;

use rerun::external::arrow::array::{
    Array as _, ArrayRef, AsArray as _, Float64Array, Int64Array, ListArray, StringArray,
    StructArray,
};
use rerun::external::arrow::buffer::OffsetBuffer;
use rerun::external::arrow::compute;
use rerun::external::arrow::datatypes::{DataType, Field, Float64Type};
use rerun::lenses::{ChunkExt as _, Lens, Selector};
use rerun::log::{Chunk, TimeColumn};
use rerun::time::TimeType;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_lenses").spawn()?;

    // region: log_data
    // Build a chunk with a struct-typed component.
    let imu_struct = StructArray::from(vec![
        (
            Field::new("x", DataType::Float64, false).into(),
            Arc::new(Float64Array::from(vec![1.0, 2.0, 3.0])) as _,
        ),
        (
            Field::new("y", DataType::Float64, false).into(),
            Arc::new(Float64Array::from(vec![4.0, 5.0, 6.0])) as _,
        ),
        (
            Field::new("elapsed", DataType::Int64, false).into(),
            Arc::new(Int64Array::from(vec![0, 10_000_000, 20_000_000])) as _,
        ),
    ]);

    let imu_list = ListArray::new(
        Field::new_list_field(imu_struct.data_type().clone(), true).into(),
        OffsetBuffer::from_lengths(std::iter::repeat_n(1, imu_struct.len())),
        Arc::new(imu_struct),
        None,
    );

    let status = StringArray::from(vec!["ok", "ok", "warn"]);
    let status_list = ListArray::new(
        Field::new_list_field(DataType::Utf8, true).into(),
        OffsetBuffer::from_lengths(std::iter::repeat_n(1, status.len())),
        Arc::new(status),
        None,
    );

    let chunk = Chunk::from_columns(
        "/sensor/imu",
        [TimeColumn::new_sequence("frame", [0, 1, 2])],
        [
            (
                rerun::ComponentDescriptor::partial("Imu:accel").with_archetype("Imu".into()),
                imu_list,
            ),
            (
                rerun::ComponentDescriptor::partial("Imu:status").with_archetype("Imu".into()),
                status_list,
            ),
        ],
    )?;
    // endregion: log_data

    // Extract the "x" field as a Scalar on the same entity.
    let extract_x = Lens::derive("Imu:accel")
        .to_component(rerun::Scalars::descriptor_scalars(), Selector::parse(".x")?)
        .build()?;

    // region: derive_lens
    // Extract the "y" field to a different entity and the "elapsed" field as a new timeline.
    let extract_y = Lens::derive("Imu:accel")
        .output_entity("/new_entity/accel_y")
        .to_component(rerun::Scalars::descriptor_scalars(), Selector::parse(".y")?)
        .to_timeline(
            "sensor_elapsed",
            TimeType::DurationNs,
            Selector::parse(".elapsed")?,
        )
        .build()?;
    // endregion: derive_lens

    // region: mutate_lens
    // Simplify the accel struct to just its "x" field in-place.
    let simplify_accel = Lens::mutate("Imu:accel", Selector::parse(".x")?).build();
    // endregion: mutate_lens

    // region: pipe_example
    // Use pipe to apply a custom transformation after extracting a field.
    let scale_x = Lens::derive("Imu:accel")
        .output_entity("/new_entity/accel_scaled_x")
        .to_component(
            rerun::Scalars::descriptor_scalars(),
            Selector::parse(".x")?.pipe(|arr: &ArrayRef| {
                let scaled: Float64Array =
                    compute::unary(arr.as_primitive::<Float64Type>(), |v| v * 9.81);
                Ok(Some(Arc::new(scaled) as _))
            }),
        )
        .build()?;
    // endregion: pipe_example

    // Apply all lenses and send the resulting chunks.
    let results = chunk
        .apply_lenses(&[extract_x, extract_y, simplify_accel, scale_x])
        .map_err(|partial| {
            let errors: Vec<_> = partial.errors().map(|e| e.to_string()).collect();
            format!("Lens errors: {}", errors.join(", "))
        })?;
    rec.send_chunks(results);

    Ok(())
}
