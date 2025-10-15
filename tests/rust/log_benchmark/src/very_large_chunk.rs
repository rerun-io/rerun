#![expect(clippy::cast_possible_wrap)]

use rerun::{Points3D, TimeColumn};

const NUM_TIMESTAMPS: usize = 4_000;
const NUM_POINTS_PER_TIMESTAMP: usize = 1024 * 128;
const NUM_POINTS: usize = NUM_TIMESTAMPS * NUM_POINTS_PER_TIMESTAMP;

/// Sends a very large chunk using `send_columns`, to make sure we can support encoding and
/// decoding >4GiB of data.
///
/// This is more of a test than a benchmark, but it would be a very costly test if ran in debug.
///
/// See <https://github.com/rerun-io/rerun/issues/11516> for more context.
pub fn run(rec: &rerun::RecordingStream) -> anyhow::Result<()> {
    re_tracing::profile_function!();

    let default_point = (1.0_f32, 2.0_f32, 3.0_f32);

    let timeline = TimeColumn::new_sequence("times", (0..NUM_TIMESTAMPS as u64).map(|v| v as i64));
    let lengths = std::iter::repeat_n(NUM_POINTS_PER_TIMESTAMP, NUM_TIMESTAMPS).collect::<Vec<_>>();

    let points = vec![default_point; NUM_POINTS];
    let columns = Points3D::new(points).columns(lengths)?;

    rec.send_columns("points", [timeline], columns)?;

    Ok(())
}
