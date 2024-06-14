//! Very minimal test of using the temporal batch APIs.

use arrow2::{
    array::{ListArray as ArrowListArray, PrimitiveArray as ArrowPrimitiveArray},
    offset::Offsets,
};
use re_chunk::ChunkTimeline;
use rerun::{components::Scalar, Loggable, Timeline};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_temporal_batch").spawn()?;

    let timeline = Timeline::new_sequence("step");
    let timeline_values: Vec<i64> = (0..64).collect();

    let scalar_data: Vec<f64> = timeline_values
        .iter()
        .map(|step| (*step as f64 / 10.0).sin())
        .collect();

    // TODO(jleibs): hide this with assorted helpers
    // ---
    let chunk_timeline =
        ArrowPrimitiveArray::<i64>::from_vec(timeline_values).to(timeline.datatype());
    let chunk_timeline = ChunkTimeline::new(None, timeline, chunk_timeline);

    let scalar_data = ArrowPrimitiveArray::<f64>::from_vec(scalar_data);
    let offsets = Offsets::try_from_lengths(std::iter::repeat(1).take(scalar_data.len()))?;
    let data_type = ArrowListArray::<i32>::default_datatype(scalar_data.data_type().clone());
    let scalar_data =
        ArrowListArray::<i32>::try_new(data_type, offsets.into(), scalar_data.boxed(), None)?;
    // ---

    let timelines = std::iter::once((timeline, chunk_timeline)).collect();
    let components = std::iter::once((Scalar::name(), scalar_data)).collect();

    rec.log_temporal_batch("scalar", timelines, components)?;

    Ok(())
}
