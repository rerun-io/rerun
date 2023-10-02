//! Log some random points with color and radii.

use rand::{distributions::Uniform, Rng as _};
use rerun::{archetypes::Points3D, components::Color, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_points3d_random").memory()?;

    let mut rng = rand::thread_rng();
    let dist = Uniform::new(-5., 5.);

    rec.log(
        "random",
        &Points3D::new((0..10).map(|_| (rng.sample(dist), rng.sample(dist), rng.sample(dist))))
            .with_colors((0..10).map(|_| Color::from_rgb(rng.gen(), rng.gen(), rng.gen())))
            .with_radii((0..10).map(|_| rng.gen::<f32>())),
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
