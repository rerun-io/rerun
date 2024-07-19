//! Log a batch of `Ellipsoids`.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_ellipsoid_batch").spawn()?;

    // Let's build a snowman!
    let belly_z = 2.5;
    let head_z = 4.5;
    rec.log(
        "batch",
        &rerun::Ellipsoids::from_centers_and_half_sizes(
            [
                (0.0, 0.0, 0.0),
                (0.0, 0.0, belly_z),
                (0.0, 0.0, head_z),
                (-0.6, -0.77, head_z),
                (0.6, -0.77, head_z),
            ],
            [
                (2.0, 2.0, 2.0),
                (1.5, 1.5, 1.5),
                (1.0, 1.0, 1.0),
                (0.15, 0.15, 0.15),
                (0.15, 0.15, 0.15),
            ],
        )
        .with_solid_colors([
            rerun::SolidColor::from_rgb(255, 255, 255),
            rerun::SolidColor::from_rgb(255, 255, 255),
            rerun::SolidColor::from_rgb(255, 255, 255),
            rerun::SolidColor::from_rgb(0, 0, 0),
            rerun::SolidColor::from_rgb(0, 0, 0),
        ]),
    )?;

    Ok(())
}
