//! Log some random points with color and radii.

use rand::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_points2d_random").spawn()?;

    let mut rng = rand::rngs::SmallRng::seed_from_u64(42);
    let dist = rand::distributions::Uniform::new(-3., 3.);

    rec.log(
        "random",
        &rerun::Points2D::new((0..10).map(|_| (rng.sample(dist), rng.sample(dist))))
            .with_colors((0..10).map(|_| rerun::Color::from_rgb(rng.gen(), rng.gen(), rng.gen())))
            .with_radii((0..10).map(|_| rng.gen::<f32>())),
    )?;

    // TODO(#5521): log VisualBounds2D

    Ok(())
}
