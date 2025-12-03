//! Logs a simple transform hierarchy with named frames.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec =
        rerun::RecordingStreamBuilder::new("rerun_example_transform3d_hierarchy_named_frames")
            .spawn()?;

    // Define entities with explicit coordinate frames.
    rec.log(
        "sun",
        &[
            &rerun::Ellipsoids3D::from_half_sizes([[1.0, 1.0, 1.0]])
                .with_colors([rerun::Color::from_rgb(255, 200, 10)])
                .with_fill_mode(rerun::FillMode::Solid) as &dyn rerun::AsComponents,
            &rerun::CoordinateFrame::new("sun_frame"),
        ],
    )?;

    rec.log(
        "planet",
        &[
            &rerun::Ellipsoids3D::from_half_sizes([[0.4, 0.4, 0.4]])
                .with_colors([rerun::Color::from_rgb(40, 80, 200)])
                .with_fill_mode(rerun::FillMode::Solid) as &dyn rerun::AsComponents,
            &rerun::CoordinateFrame::new("planet_frame"),
        ],
    )?;

    rec.log(
        "moon",
        &[
            &rerun::Ellipsoids3D::from_half_sizes([[0.15, 0.15, 0.15]])
                .with_colors([rerun::Color::from_rgb(180, 180, 180)])
                .with_fill_mode(rerun::FillMode::Solid) as &dyn rerun::AsComponents,
            &rerun::CoordinateFrame::new("moon_frame"),
        ],
    )?;

    // Define explicit frame relationships.
    rec.log(
        "planet_transform",
        &rerun::Transform3D::from_translation([6.0, 0.0, 0.0])
            .with_child_frame("planet_frame")
            .with_parent_frame("sun_frame"),
    )?;

    rec.log(
        "moon_transform",
        &rerun::Transform3D::from_translation([3.0, 0.0, 0.0])
            .with_child_frame("moon_frame")
            .with_parent_frame("planet_frame"),
    )?;

    // Connect the viewer to the sun's coordinate frame.
    // This is only needed in the absence of blueprints since a default view will typically be created at `/`.
    rec.log_static("/", &rerun::CoordinateFrame::new("sun_frame"))?;

    Ok(())
}
