//! Log lines with ui points & scene unit radii.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_line_strip2d_ui_radius").spawn()?;

    // A blue line with a scene unit radii of 0.01.
    let points = [[0., 0.], [0., 1.], [1., 0.], [1., 1.]];
    rec.log(
        "scene_unit_line",
        &rerun::LineStrips2D::new([points])
            // By default, radii are interpreted as world-space units.
            .with_radii([0.01])
            .with_colors([rerun::Color::from_rgb(0, 0, 255)]),
    )?;

    // A red line with a ui point radii of 5.
    // Ui points are independent of zooming in Views, but are sensitive to the application ui scaling.
    // For 100 % ui scaling, ui points are equal to pixels.
    let points = [[3., 0.], [3., 1.], [4., 0.], [4., 1.]];
    rec.log(
        "ui_points_line",
        &rerun::LineStrips2D::new([points])
            // rerun::Radius::new_ui_points produces a radius that the viewer interprets as given in ui points.
            .with_radii([rerun::Radius::new_ui_points(5.0)])
            .with_colors([rerun::Color::from_rgb(255, 0, 0)]),
    )?;

    // TODO(#5520): log VisualBounds2D

    Ok(())
}
