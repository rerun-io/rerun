//! Log a simple colored triangle.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_mesh3d").spawn()?;

    rec.log(
        "triangle",
        &rerun::Mesh3D::new([[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]])
            .with_vertex_normals([[0.0, 0.0, 1.0]])
            .with_vertex_colors([0xFF0000FF, 0x00FF00FF, 0x0000FFFF]),
    )?;

    Ok(())
}
