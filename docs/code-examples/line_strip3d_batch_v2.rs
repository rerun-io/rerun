//! Log a batch of 2d line strips.

use rerun::{archetypes::LineStrips3D, MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) = RecordingStreamBuilder::new(env!("CARGO_BIN_NAME")).memory()?;

    let strip1 = [[0., 0., 2.], [1., 0., 2.], [1., 1., 2.], [0., 1., 2.]];
    let strip2 = [
        [0., 0., 0.],
        [0., 0., 1.],
        [1., 0., 0.],
        [1., 0., 1.],
        [1., 1., 0.],
        [1., 1., 1.],
        [0., 1., 0.],
        [0., 1., 1.],
    ];
    MsgSender::from_archetype(
        "strips",
        &LineStrips3D::new([strip1.to_vec(), strip2.to_vec()])
            .with_colors([0xFF0000FF, 0x00FF00FF])
            .with_radii([0.025, 0.005]),
    )?
    .send(&rec_stream)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
