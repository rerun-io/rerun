//! Update a set of vectors over time.
//!
//! See also the `arrows3d_column_updates` example, which achieves the same thing in a single operation.

use rerun::demo_util::linspace;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_arrows3d_row_updates").spawn()?;

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

    for (time, origins, vectors, color) in itertools::izip!(10..15, origins, vectors, colors) {
        rec.set_time_seconds("time", time);

        let arrows = rerun::Arrows3D::from_vectors(vectors)
            .with_origins(origins)
            .with_colors([color]);

        rec.log("arrows", &arrows)?;
    }

    Ok(())
}
