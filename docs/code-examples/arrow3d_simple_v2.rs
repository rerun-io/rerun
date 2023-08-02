//! Log a batch of 3D arrows.

use std::f32::consts::TAU;

// TODO(cmc): use `rerun::components::Arrow3D` once migration is done.
use rerun::external::re_types::components::Arrow3D;
use rerun::{
    archetypes::Arrows3D, components::Color, datatypes::Vec3D, MsgSender, RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) = RecordingStreamBuilder::new("arrow").memory()?;

    let (arrows, colors): (Vec<_>, Vec<_>) = (0..100)
        .map(|i| {
            let angle = TAU * i as f32 * 0.01;
            (
                {
                    let length = ((i + 1) as f32).log2();
                    Arrow3D::new(
                        Vec3D::ZERO,
                        [length * angle.sin(), 0.0, length * angle.cos()],
                    )
                },
                {
                    let c = (angle / TAU * 255.0) as u8;
                    Color::from_unmultiplied_rgba(255 - c, c, 128, 128)
                },
            )
        })
        .unzip();

    MsgSender::from_archetype("arrows", &Arrows3D::new(arrows).with_colors(colors))?
        .send(&rec_stream)?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
