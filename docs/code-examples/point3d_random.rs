//! Log some random points with color and radii."""
use rand::distributions::Uniform;
use rand::Rng;
use rerun::components::{Color, Point3D, Radius};
use rerun::{MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) = RecordingStreamBuilder::new("rerun-example-points").memory()?;

    let mut rng = rand::thread_rng();
    let position_distribs = Uniform::new(-5., 5.);

    let mut positions = vec![];
    let mut colors = vec![];
    let mut radii = vec![];
    for _ in 0..10 {
        positions.push(Point3D::new(
            rng.sample(position_distribs),
            rng.sample(position_distribs),
            rng.sample(position_distribs),
        ));
        colors.push(Color::from_rgb(rng.gen(), rng.gen(), rng.gen()));
        radii.push(Radius(rng.gen()));
    }

    MsgSender::new("random")
        .with_component(&positions)?
        .with_component(&colors)?
        .with_component(&radii)?
        .send(&rec_stream)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
