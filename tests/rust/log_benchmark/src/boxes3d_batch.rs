use std::ops::RangeInclusive;

use rerun::external::re_log;

const W: usize = 200;
const H: usize = 200;
const MIN_Z: usize = 10;
const MAX_Z: usize = 20;

struct Input {
    centers: Vec<glam::Vec3>,
}

// TODO: use emath::remap instead (when I'm not on an airplane…).
fn remap(from: f32, from_range: RangeInclusive<f32>, to_range: RangeInclusive<f32>) -> f32 {
    let normalized = (from - *from_range.start()) / (from_range.end() - from_range.start());
    *to_range.start() + (to_range.end() - to_range.start()) * normalized
}

fn prepare() -> Input {
    let mut centers = vec![];
    for x in 0..W {
        for y in 0..H {
            let height = remap(
                ((x + y) as f32 * 0.1).sin(),
                -1.0..=1.0,
                MIN_Z as f32..=MAX_Z as f32,
            );
            for z in 0..height.round() as usize {
                centers.push(glam::Vec3::new(x as f32, y as f32, z as f32));
            }
        }
    }

    re_log::info!("Logging {} boxes", centers.len());

    Input { centers }
}

fn execute(rec: &rerun::RecordingStream, input: Input) -> anyhow::Result<()> {
    re_tracing::profile_function!();

    let Input { centers } = input;

    rec.log(
        "large_batch",
        &rerun::Boxes3D::update_fields()
            .with_half_sizes([rerun::HalfSize3D::new(0.5, 0.5, 0.5)])
            .with_centers(centers)
            .with_fill_mode(rerun::FillMode::Solid),
    )?;
    Ok(())
}

/// Emulate a voxel occupancy grid
pub fn run(rec: &rerun::RecordingStream) -> anyhow::Result<()> {
    re_tracing::profile_function!();
    let input = std::hint::black_box(prepare());
    execute(rec, input)
}
