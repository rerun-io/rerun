//! Log random points and the corresponding covariance ellipsoid.

use rand::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_ellipsoid_simple").spawn()?;

    let sigmas: [f32; 3] = [5., 3., 1.];

    let mut rng = rand::rngs::SmallRng::seed_from_u64(42);
    let normal = rand_distr::Normal::new(0.0, 1.0)?;

    rec.log(
        "points",
        &rerun::Points3D::new((0..50_000).map(|_| {
            (
                sigmas[0] * normal.sample(&mut rng),
                sigmas[1] * normal.sample(&mut rng),
                sigmas[2] * normal.sample(&mut rng),
            )
        }))
        .with_radii([0.02])
        .with_colors([rerun::Color::from_rgb(188, 77, 185)]),
    )?;

    rec.log(
        "ellipsoid",
        &rerun::Ellipsoids3D::from_centers_and_half_sizes(
            [(0.0, 0.0, 0.0), (0.0, 0.0, 0.0)],
            [sigmas, [sigmas[0] * 3., sigmas[1] * 3., sigmas[2] * 3.]],
        )
        .with_colors([
            rerun::Color::from_rgb(255, 255, 0),
            rerun::Color::from_rgb(64, 64, 0),
        ]),
    )?;

    Ok(())
}
