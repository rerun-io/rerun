//! Logs a simple transform hierarchy.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec =
        rerun::RecordingStreamBuilder::new("rerun_example_transform3d_hierarchy_simple").spawn()?;

    // Log entities at their hierarchy positions.
    rec.log(
        "sun",
        &rerun::Ellipsoids3D::from_half_sizes([[1.0, 1.0, 1.0]])
            .with_colors([rerun::Color::from_rgb(255, 200, 10)])
            .with_fill_mode(rerun::FillMode::Solid),
    )?;

    rec.log(
        "sun/planet",
        &rerun::Ellipsoids3D::from_half_sizes([[0.4, 0.4, 0.4]])
            .with_colors([rerun::Color::from_rgb(40, 80, 200)])
            .with_fill_mode(rerun::FillMode::Solid),
    )?;

    rec.log(
        "sun/planet/moon",
        &rerun::Ellipsoids3D::from_half_sizes([[0.15, 0.15, 0.15]])
            .with_colors([rerun::Color::from_rgb(180, 180, 180)])
            .with_fill_mode(rerun::FillMode::Solid),
    )?;

    // Define transforms - each describes the relationship to its parent.
    rec.log(
        "sun/planet",
        &rerun::Transform3D::from_translation([6.0, 0.0, 0.0]),
    )?; // Planet 6 units from sun.
    rec.log(
        "sun/planet/moon",
        &rerun::Transform3D::from_translation([3.0, 0.0, 0.0]),
    )?; // Moon 3 units from planet.

    Ok(())
}
