//! Log some random points with color and radii.

use rand::{distributions::Uniform, Rng as _};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) =
        rerun::RecordingStreamBuilder::new("rerun_example_points3d_random").memory()?;

    let mut rng = rand::thread_rng();
    let dist = Uniform::new(-5., 5.);

    rec.log(
        "random",
        &rerun::Points3D::new(
            (0..10).map(|_| (rng.sample(dist), rng.sample(dist), rng.sample(dist))),
        )
        .with_colors((0..10).map(|_| rerun::Color::from_rgb(rng.gen(), rng.gen(), rng.gen())))
        .with_radii((0..10).map(|_| rng.gen::<f32>())),
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
