//! Log a small volume: a soft sphere sampled on a 32³ grid.

use half::f16;
use ndarray::Array3;

const SIZE: usize = 32;
const RADIUS: f32 = 14.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec =
        rerun::RecordingStreamBuilder::new("rerun_example_volume3d_simple")
            .spawn()?;

    // Dimensions are ordered `[z, y, x]`. Only `f16` values are supported for now.
    let values = Array3::from_shape_fn((SIZE, SIZE, SIZE), |(z, y, x)| {
        // Squared distance from the center of the grid, in voxel units.
        let center = 0.5 * (SIZE - 1) as f32;
        let [dx, dy, dz] = [x, y, z].map(|i| i as f32 - center);
        let distance_sq = dx * dx + dy * dy + dz * dz;

        // A soft sphere: 1 at the center, falling off to 0 at `RADIUS`.
        f16::from_f32((1.0 - distance_sq / (RADIUS * RADIUS)).max(0.0))
    });

    rec.log(
        "volume",
        &rerun::Volume3D::new(rerun::encodings::TensorData::try_from(values)?)
            .with_voxel_size([0.1, 0.1, 0.1]),
    )?;

    Ok(())
}
