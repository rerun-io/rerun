//! Log some random points with color and radii.

use rand::distributions::Uniform;
use rand::Rng;
use rerun::{archetypes::Points3D, components::Color, MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) =
        RecordingStreamBuilder::new("rerun-example-points3d_random").memory()?;

    let mut rng = rand::thread_rng();
    let dist = Uniform::new(-5., 5.);

    MsgSender::from_archetype(
        "random",
        &Points3D::new((0..10).map(|_| (rng.sample(dist), rng.sample(dist), rng.sample(dist))))
            .with_colors((0..10).map(|_| Color::from_rgb(rng.gen(), rng.gen(), rng.gen())))
            .with_radii((0..10).map(|_| rng.gen::<f32>())),
    )?
    .send(&rec_stream)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
