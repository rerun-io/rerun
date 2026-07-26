//! This example demonstrates how to log procedurally generated raw 3D meshes
//! (so-called "triangle soups") and a transform hierarchy.
//!
//! Usage:
//! ```
//! cargo run -p raw_mesh
//! ```

use rerun::external::re_log;
use rerun::{Color, Mesh3D, RecordingStream, Rgba32, Transform3D};

type Vec3 = [f32; 3];
type Triangle = [usize; 3];

fn triangle_soup(vertices: &[Vec3], triangles: &[Triangle], face_colors: &[Color]) -> Mesh3D {
    assert_eq!(triangles.len(), face_colors.len());

    let mut positions = Vec::with_capacity(triangles.len() * 3);
    let mut normals = Vec::with_capacity(triangles.len() * 3);
    let mut colors = Vec::with_capacity(triangles.len() * 3);

    for (triangle, color) in triangles.iter().zip(face_colors) {
        let [p0, p1, p2] = triangle.map(|index| vertices[index]);
        let edge1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        let edge2 = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
        let normal = [
            edge1[1] * edge2[2] - edge1[2] * edge2[1],
            edge1[2] * edge2[0] - edge1[0] * edge2[2],
            edge1[0] * edge2[1] - edge1[1] * edge2[0],
        ];
        let length = (normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt();
        let unit_normal = [normal[0] / length, normal[1] / length, normal[2] / length];

        positions.extend([p0, p1, p2]);
        normals.extend([unit_normal; 3]);
        colors.extend([*color; 3]);
    }

    Mesh3D::new(positions)
        .with_vertex_normals(normals)
        .with_vertex_colors(colors)
}

fn box_mesh(size: Vec3) -> Mesh3D {
    let [x, y, z] = size.map(|dimension| dimension / 2.0);
    let vertices = [
        [-x, -y, -z],
        [x, -y, -z],
        [x, y, -z],
        [-x, y, -z],
        [-x, -y, z],
        [x, -y, z],
        [x, y, z],
        [-x, y, z],
    ];
    let triangles = [
        [0, 2, 1],
        [0, 3, 2],
        [4, 5, 6],
        [4, 6, 7],
        [0, 1, 5],
        [0, 5, 4],
        [1, 2, 6],
        [1, 6, 5],
        [2, 3, 7],
        [2, 7, 6],
        [3, 0, 4],
        [3, 4, 7],
    ];
    let face_colors = [
        Color::from_rgb(95, 145, 255),
        Color::from_rgb(95, 145, 255),
        Color::from_rgb(80, 120, 220),
        Color::from_rgb(80, 120, 220),
        Color::from_rgb(120, 165, 255),
        Color::from_rgb(120, 165, 255),
        Color::from_rgb(65, 105, 205),
        Color::from_rgb(65, 105, 205),
        Color::from_rgb(110, 155, 245),
        Color::from_rgb(110, 155, 245),
        Color::from_rgb(75, 115, 215),
        Color::from_rgb(75, 115, 215),
    ];

    triangle_soup(&vertices, &triangles, &face_colors)
}

fn pyramid_mesh() -> Mesh3D {
    Mesh3D::new([
        [-0.45, -0.45, 0.0],
        [0.45, -0.45, 0.0],
        [0.45, 0.45, 0.0],
        [-0.45, 0.45, 0.0],
        [0.0, 0.0, 0.8],
    ])
    .with_triangle_indices([
        [0, 2, 1],
        [0, 3, 2],
        [0, 1, 4],
        [1, 2, 4],
        [2, 3, 4],
        [3, 0, 4],
    ])
    .with_albedo_factor(Rgba32::from_rgb(255, 170, 60))
}

fn log_scene(rec: &RecordingStream) -> anyhow::Result<()> {
    rec.log_static("world", &rerun::ViewCoordinates::RFU())?;

    rec.log_static("world/base", &box_mesh([2.6, 1.8, 0.35]))?;

    rec.log_static(
        "world/base/arm",
        &Transform3D::from_translation([0.0, 0.0, 0.9]),
    )?;
    rec.log_static("world/base/arm", &box_mesh([0.45, 0.45, 1.5]))?;

    rec.log_static(
        "world/base/arm/tool",
        &Transform3D::from_translation([0.0, 0.0, 1.1]),
    )?;
    rec.log_static("world/base/arm/tool", &pyramid_mesh())?;

    Ok(())
}

#[derive(Debug, clap::Parser)]
#[clap(author, version, about)]
struct Args {
    #[command(flatten)]
    rerun: rerun::clap::RerunArgs,
}

fn main() -> anyhow::Result<()> {
    re_log::setup_logging();

    use clap::Parser as _;
    let args = Args::parse();

    let (rec, _serve_guard) = args.rerun.init("rerun_example_raw_mesh")?;
    log_scene(&rec)
}
