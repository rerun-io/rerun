//! Log several 3D geometry primitives.

use rerun::external::glam::vec3;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_geometry3d_primitives").spawn()?;

    rec.log(
        "cones",
        &rerun::Cones3D::from_lengths_and_radii([1.6, 2.2, 1.2], [0.6, 0.35, 0.8])
            .with_centers([vec3(-2.0, 0.0, 0.0), vec3(-0.5, 0.0, 0.3), vec3(1.0, 0.0, -0.2)])
            .with_colors([
                rerun::Color::from_rgb(255, 120, 80),
                rerun::Color::from_rgb(255, 210, 80),
                rerun::Color::from_rgb(120, 200, 255),
            ]),
    )?;

    rec.log(
        "rays",
        &rerun::Rays3D::from_vectors([
            vec3(0.8, 0.7, 0.4),
            vec3(0.4, 1.0, 0.8),
            vec3(1.0, 0.4, 0.2),
        ])
        .with_origins([
            vec3(-2.7, -1.5, 0.0),
            vec3(-1.2, -1.5, 0.0),
            vec3(0.3, -1.5, 0.0),
        ])
        .with_radii([0.025])
        .with_colors([rerun::Color::from_rgb(80, 220, 180)]),
    )?;

    rec.log(
        "planes",
        &rerun::Planes3D::from_planes([
            rerun::components::Plane3D::XY.with_distance(-0.75),
            rerun::components::Plane3D::new([0.5, 0.0, 1.0], 0.2),
        ])
        .with_half_sizes([(2.5, 1.2), (1.4, 1.0)])
        .with_colors([
            rerun::Color::from_unmultiplied_rgba(120, 120, 255, 96),
            rerun::Color::from_unmultiplied_rgba(255, 160, 80, 96),
        ])
        .with_fill_mode(rerun::FillMode::TransparentFillMajorWireframe),
    )?;

    rec.log(
        "triangles",
        &rerun::Triangles3D::from_vertices([
            vec3(-2.0, 1.4, 0.0),
            vec3(-1.0, 1.4, 0.0),
            vec3(-1.5, 2.2, 0.6),
            vec3(0.0, 1.4, 0.0),
            vec3(1.0, 1.4, 0.0),
            vec3(0.5, 2.2, 0.6),
        ])
        .with_colors([
            rerun::Color::from_rgb(255, 80, 140),
            rerun::Color::from_rgb(120, 255, 120),
        ])
        .with_fill_mode(rerun::FillMode::TransparentFillMajorWireframe),
    )?;

    Ok(())
}
