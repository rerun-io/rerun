//! Demonstrates using explicit `CoordinateFrame` with implicit transform frames only.

#![expect(clippy::cast_possible_wrap)]

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_transform3d_hierarchy").spawn()?;

    rec.set_time_sequence("time", 0);
    rec.log(
        "red_box",
        &[
            &rerun::Boxes3D::from_half_sizes([(0.5, 0.5, 0.5)]).with_colors([(255, 0, 0)])
                as &dyn rerun::AsComponents,
            // Use Transform3D to place the box, so we actually change the underlying coordinate frame and not just the box's pose.
            &rerun::Transform3D::from_translation([2.0, 0.0, 0.0]),
        ],
    )?;
    rec.log(
        "blue_box",
        &[
            &rerun::Boxes3D::from_half_sizes([(0.5, 0.5, 0.5)]).with_colors([(0, 0, 255)])
                as &dyn rerun::AsComponents,
            // Use Transform3D to place the box, so we actually change the underlying coordinate frame and not just the box's pose.
            &rerun::Transform3D::from_translation([-2.0, 0.0, 0.0]),
        ],
    )?;
    rec.log(
        "point",
        &rerun::Points3D::new([(0.0, 0.0, 0.0)]).with_radii([0.5]),
    )?;

    // Change where the point is located by cycling through its coordinate frame.
    for (t, frame_id) in ["tf#/red_box", "tf#/blue_box"].into_iter().enumerate() {
        rec.set_time_sequence("time", t as i64 + 1); // leave it untouched at t==0.
        rec.log("point", &rerun::CoordinateFrame::new(frame_id))?;
    }

    Ok(())
}
