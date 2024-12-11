//! Log a batch of 3D arrows.

use std::f32::consts::TAU;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_arrow3d").spawn()?;

    let origins = vec![rerun::Position3D::ZERO; 100];
    let (vectors, colors): (Vec<_>, Vec<_>) = (0..100)
        .map(|i| {
            let angle = TAU * i as f32 * 0.01;
            let length = ((i + 1) as f32).log2();
            let c = (angle / TAU * 255.0).round() as u8;
            (
                rerun::Vector3D::from([(length * angle.sin()), 0.0, (length * angle.cos())]),
                rerun::Color::from_unmultiplied_rgba(255 - c, c, 128, 128),
            )
        })
        .unzip();

    rec.log(
        "arrows",
        &rerun::Arrows3D::from_vectors(vectors)
            .with_origins(origins)
            .with_colors(colors),
    )?;

    Ok(())
}
