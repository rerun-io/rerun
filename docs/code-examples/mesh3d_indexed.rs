//! Log a simple colored triangle with indexed drawing.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_mesh3d_indexed")
        .spawn(rerun::default_flush_timeout())?;

    rec.log(
        "triangle",
        &rerun::Mesh3D::new([[0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 0.0]])
            .with_vertex_normals([[0.0, 0.0, 1.0]])
            .with_vertex_colors([0x0000FFFF, 0x00FF00FF, 0xFF0000FF])
            .with_mesh_properties(rerun::MeshProperties::from_triangle_indices([[2, 1, 0]]))
            .with_mesh_material(rerun::Material::from_albedo_factor(0xCC00CCFF)),
    )?;

    Ok(())
}
