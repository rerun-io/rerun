//! Log some random points with color and radii.
use rand::distributions::Uniform;
use rand::Rng;
use rerun::components::{ColorRGBA, LegacyPoint2D, Radius, Rect2D, Vec4D};
use rerun::{MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) = RecordingStreamBuilder::new("points").memory()?;

    let mut rng = rand::thread_rng();
    let position_distribs = Uniform::new(-3., 3.);

    let mut positions = vec![];
    let mut colors = vec![];
    let mut radii = vec![];
    for _ in 0..10 {
        positions.push(LegacyPoint2D::new(
            rng.sample(position_distribs),
            rng.sample(position_distribs),
        ));
        colors.push(ColorRGBA::from_rgb(rng.gen(), rng.gen(), rng.gen()));
        radii.push(Radius(rng.gen()));
    }

    MsgSender::new("random")
        .with_component(&positions)?
        .with_component(&colors)?
        .with_component(&radii)?
        .send(&rec_stream)?;

    // Log an extra rect to set the view bounds
    MsgSender::new("bounds")
        .with_component(&[Rect2D::XCYCWH(Vec4D([0.0, 0.0, 8.0, 6.0]))])?
        .send(&rec_stream)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
