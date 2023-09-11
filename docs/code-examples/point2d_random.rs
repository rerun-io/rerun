//! Log some random points with color and radii.

use rand::distributions::Uniform;
use rand::Rng;
use rerun::{
    archetypes::{Boxes2D, Points2D},
    components::Color,
    RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_points2d").memory()?;

    let mut rng = rand::thread_rng();
    let dist = Uniform::new(-3., 3.);

    rec.log(
        "random",
        &Points2D::new((0..10).map(|_| (rng.sample(dist), rng.sample(dist))))
            .with_colors((0..10).map(|_| Color::from_rgb(rng.gen(), rng.gen(), rng.gen())))
            .with_radii((0..10).map(|_| rng.gen::<f32>())),
    )?;

    // Log an extra rect to set the view bounds
    rec.log("bounds", &Boxes2D::new([(4.0, 3.0)]))?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
