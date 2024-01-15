//! Log some random points with color and radii.

use rand::{distributions::Uniform, Rng as _};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_points2d").spawn()?;

    let mut rng = rand::thread_rng();
    let dist = Uniform::new(-3., 3.);

    rec.log(
        "random",
        &rerun::Points2D::new((0..10).map(|_| (rng.sample(dist), rng.sample(dist))))
            .with_colors((0..10).map(|_| rerun::Color::from_rgb(rng.gen(), rng.gen(), rng.gen())))
            .with_radii((0..10).map(|_| rng.gen::<f32>())),
    )?;

    // Log an extra rect to set the view bounds
    rec.log("bounds", &rerun::Boxes2D::from_half_sizes([(4., 3.)]))?;

    Ok(())
}
