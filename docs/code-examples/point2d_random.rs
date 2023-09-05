//! Log some random points with color and radii.

use rand::distributions::Uniform;
use rand::Rng;
use rerun::{
    archetypes::Points2D,
    components::{Color, Rect2D},
    datatypes::Vec4D,
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
    // TODO(#2786): Rect2D archetype
    rec.log_component_lists(
        "bounds",
        false,
        1,
        [&Rect2D::XCYCWH(Vec4D([0.0, 0.0, 8.0, 6.0]).into()) as _],
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
