//! Logs a transform hierarchy using named transform frame relationships.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec =
        rerun::RecordingStreamBuilder::new("rerun_example_transform3d_hierarchy_frames").spawn()?;

    rec.set_duration_secs("sim_time", 0.0);

    // Planetary motion is typically in the XY plane.
    rec.log_static("/", &rerun::ViewCoordinates::RIGHT_HAND_Z_UP())?;

    // Setup spheres, all are in the center of their own space:
    rec.log(
        "sun",
        &[
            &rerun::Ellipsoids3D::from_centers_and_half_sizes([[0.0, 0.0, 0.0]], [[1.0, 1.0, 1.0]])
                .with_colors([rerun::Color::from_rgb(255, 200, 10)])
                .with_fill_mode(rerun::components::FillMode::Solid)
                as &dyn rerun::AsComponents,
            &rerun::CoordinateFrame::new("sun_frame"),
        ],
    )?;

    rec.log(
        "planet",
        &[
            &rerun::Ellipsoids3D::from_centers_and_half_sizes([[0.0, 0.0, 0.0]], [[0.4, 0.4, 0.4]])
                .with_colors([rerun::Color::from_rgb(40, 80, 200)])
                .with_fill_mode(rerun::components::FillMode::Solid)
                as &dyn rerun::AsComponents,
            &rerun::CoordinateFrame::new("planet_frame"),
        ],
    )?;

    rec.log(
        "moon",
        &[
            &rerun::Ellipsoids3D::from_centers_and_half_sizes(
                [[0.0, 0.0, 0.0]],
                [[0.15, 0.15, 0.15]],
            )
            .with_colors([rerun::Color::from_rgb(180, 180, 180)])
            .with_fill_mode(rerun::components::FillMode::Solid)
                as &dyn rerun::AsComponents,
            &rerun::CoordinateFrame::new("moon_frame"),
        ],
    )?;

    // The viewer automatically creates a 3D view at `/`. To connect it to our transform hierarchy, we set its coordinate frame
    // to `sun_frame` as well. Alternatively, we could also set a blueprint that makes `/sun` the space origin.
    rec.log("/", &rerun::CoordinateFrame::new("sun_frame"))?;

    // Draw fixed paths where the planet & moon move.
    let d_planet = 6.0;
    let d_moon = 3.0;
    let angles = (0..=100).map(|i| i as f32 * 0.01 * std::f32::consts::TAU);
    let circle: Vec<_> = angles.map(|angle| [angle.sin(), angle.cos()]).collect();
    rec.log(
        "planet_path",
        &[
            &rerun::LineStrips3D::new([rerun::LineStrip3D::from_iter(
                circle
                    .iter()
                    .map(|p| [p[0] * d_planet, p[1] * d_planet, 0.0]),
            )]) as &dyn rerun::AsComponents,
            &rerun::CoordinateFrame::new("sun_frame"),
        ],
    )?;
    rec.log(
        "moon_path",
        &[
            &rerun::LineStrips3D::new([rerun::LineStrip3D::from_iter(
                circle.iter().map(|p| [p[0] * d_moon, p[1] * d_moon, 0.0]),
            )]) as &dyn rerun::AsComponents,
            &rerun::CoordinateFrame::new("planet_frame"),
        ],
    )?;

    // Movement via transforms.
    for i in 0..(6 * 120) {
        let time = i as f32 / 120.0;
        rec.set_duration_secs("sim_time", time);
        let r_moon = time * 5.0;
        let r_planet = time * 2.0;

        rec.log(
            "planet_transforms",
            &rerun::Transform3D::from_translation_rotation(
                [r_planet.sin() * d_planet, r_planet.cos() * d_planet, 0.0],
                rerun::RotationAxisAngle {
                    axis: [1.0, 0.0, 0.0].into(),
                    angle: rerun::Angle::from_degrees(20.0),
                },
            )
            .with_child_frame("planet_frame")
            .with_parent_frame("sun_frame"),
        )?;
        rec.log(
            "moon_transforms",
            &rerun::Transform3D::from_translation([
                r_moon.cos() * d_moon,
                r_moon.sin() * d_moon,
                0.0,
            ])
            .with_relation(rerun::TransformRelation::ChildFromParent)
            .with_child_frame("moon_frame")
            .with_parent_frame("planet_frame"),
        )?;
    }

    Ok(())
}
