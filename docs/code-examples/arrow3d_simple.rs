//! Log a batch of 3D arrows.

use std::f64::consts::TAU;

use rerun::{
    archetypes::Arrows3D,
    components::{Color, Vector3D},
    RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_arrow3d").memory()?;

    let (vectors, colors): (Vec<_>, Vec<_>) = (0..100)
        .map(|i| {
            let angle = rnd(TAU * i as f64 * 0.01);
            let length = rnd(((i + 1) as f64).log2());
            let c = (angle / TAU * 255.0).round() as u8;
            (
                Vector3D::from([
                    (length * angle.sin()) as f32,
                    0.0,
                    (length * angle.cos()) as f32,
                ]),
                Color::from_unmultiplied_rgba(255 - c, c, 128, 128),
            )
        })
        .unzip();

    rec.log("arrows", &Arrows3D::new(vectors).with_colors(colors))?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}

fn rnd(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
