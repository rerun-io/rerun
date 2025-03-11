//! Update a set of vectors over time, in a single operation.
//!
//! This is semantically equivalent to the `arrows3d_row_updates` example, albeit much faster.

use rerun::demo_util::linspace;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec =
        rerun::RecordingStreamBuilder::new("rerun_example_arrows3d_column_updates").spawn()?;
    let times = rerun::TimeColumn::new_duration_seconds("time", 10..15);

    // Prepare a fixed sequence of arrows over 5 timesteps.
    // Origins stay constant, vectors change magnitude and direction, and each timestep has a unique color.
    let (origins, vectors): (Vec<_>, Vec<_>) = (0..5)
        .map(|i| {
            let i = i as f32;
            (
                linspace(-1., 1., 5).map(move |x| (x, x, 0.)),
                linspace(-1., 1., 5)
                    .zip(linspace(i / 10., i, 5))
                    .map(|(x, z)| (x, x, z)),
            )
        })
        .collect();

    // At each timestep, all arrows share the same but changing color.
    let colors = [0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFF00FF, 0x00FFFFFF];

    let arrows = rerun::Arrows3D::update_fields()
        .with_origins(origins.into_iter().flatten())
        .with_vectors(vectors.into_iter().flatten())
        .columns([5, 5, 5, 5, 5])?;
    let color = rerun::Arrows3D::update_fields()
        .with_colors(colors)
        .columns_of_unit_batches()?;

    rec.send_columns("arrows", [times], arrows.chain(color))?;

    Ok(())
}
