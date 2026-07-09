//! Use a blueprint to customize a Spatial3DView.

use rerun::blueprint::{
    Blueprint, Spatial3DView, archetypes as blueprint_archetypes,
    components as blueprint_components,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let blueprint = Blueprint::new(
        Spatial3DView::new("3D Scene")
            .with_origin("/")
            .with_background(rerun::Color::from_rgb(100, 149, 237))
            .with_eye_controls(
                blueprint_archetypes::EyeControls3D::new()
                    .with_position((0.0, 0.0, 2.0))
                    .with_look_target((0.0, 2.0, 0.0))
                    .with_eye_up((-1.0, 0.0, 0.0))
                    .with_spin_speed(0.2)
                    .with_kind(blueprint_components::Eye3DKind::FirstPerson)
                    .with_speed(20.0),
            )
            .with_line_grid(
                blueprint_archetypes::LineGrid3D::new()
                    .with_visible(true)
                    .with_spacing(0.1)
                    .with_plane(rerun::components::Plane3D::new(
                        [0.0, 0.0, 1.0],
                        -5.0,
                    ))
                    .with_stroke_width(2.0)
                    .with_color([255, 255, 255, 128]),
            )
            .with_spatial_information(
                blueprint_archetypes::SpatialInformation::new("tf#/")
                    .with_show_axes(true)
                    .with_show_bounding_box(true),
            ),
    );

    let rec = rerun::RecordingStreamBuilder::new("rerun_example_spatial_3d")
        .with_blueprint(blueprint)
        .spawn()?;

    let positions = (0..50).map(|i| {
        let i = i as f64;
        [
            (i * 0.37).sin() * 4.0,
            (i * 0.21).cos() * 4.0,
            -5.0 + i * 10.0 / 49.0,
        ]
    });
    let colors = (0..50_i32).map(|i| {
        [
            ((i * 53) % 255) as u8,
            ((128 + i * 29) % 255) as u8,
            (255 - i * 17).rem_euclid(255) as u8,
        ]
    });
    let radii = (0..50).map(|i| (0.1 + (0.5 - 0.1) * i as f64 / 49.0) as f32);

    rec.log(
        "points",
        &rerun::Points3D::new(positions)
            .with_colors(colors)
            .with_radii(radii),
    )?;
    rec.log(
        "box",
        &rerun::Boxes3D::from_half_sizes([(5.0, 5.0, 5.0)]).with_colors([0]),
    )?;

    Ok(())
}
