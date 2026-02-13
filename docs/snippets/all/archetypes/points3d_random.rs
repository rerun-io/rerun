//! Log some random points with color and radii.

use rand::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_points3d_random").spawn()?;

    let mut rng = rand::rngs::SmallRng::seed_from_u64(42);
    let dist = rand::distr::Uniform::new(-5., 5.)?;

    rec.log(
        "random",
        &rerun::Points3D::new(
            (0..10).map(|_| (rng.sample(dist), rng.sample(dist), rng.sample(dist))),
        )
        .with_colors(
            (0..10).map(|_| rerun::Color::from_rgb(rng.random(), rng.random(), rng.random())),
        )
        .with_radii((0..10).map(|_| rng.random::<f32>())),
    )?;

    Ok(())
}
