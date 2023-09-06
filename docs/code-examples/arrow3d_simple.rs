//! Log a batch of 3D arrows.

use std::f32::consts::TAU;

use rerun::{
    archetypes::Arrows3D,
    components::{Color, Vector3D},
    RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_arrow3d").memory()?;

    let (vectors, colors): (Vec<_>, Vec<_>) = (0..100)
        .map(|i| {
            let angle = TAU * i as f32 * 0.01;
            let length = ((i + 1) as f32).log2();
            let c = (angle / TAU * 255.0).round() as u8;
            (
                Vector3D::from([(length * angle.sin()), 0.0, (length * angle.cos())]),
                Color::from_unmultiplied_rgba(255 - c, c, 128, 128),
            )
        })
        .unzip();

    rec.log("arrows", &Arrows3D::new(vectors).with_colors(colors))?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
