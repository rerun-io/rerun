//! Log a pinhole and a random image.
use ndarray::{Array, ShapeBuilder};
use rerun::components::{Mat3x3, Pinhole, Tensor, Vec2D};
use rerun::{MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) = RecordingStreamBuilder::new("pinhole").memory()?;

    let mut image = Array::<u8, _>::default((3, 3, 3).f());
    image.map_inplace(|x| *x = rand::random());

    MsgSender::new("world/image")
        .with_component(&[Pinhole {
            image_from_cam: Mat3x3::from([[3., 0., 1.5], [0., 3., 1.5], [0., 0., 1.]]),
            resolution: Some(Vec2D::from([3., 3.])),
        }])?
        .send(&rec_stream)?;

    MsgSender::new("world/image")
        .with_component(&[Tensor::try_from(image.as_standard_layout().view())?])?
        .send(&rec_stream)?;

    rec_stream.flush_blocking();
    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
