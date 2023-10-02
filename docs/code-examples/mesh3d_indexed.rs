//! Log a simple colored triangle with indexed drawing.

use rerun::{
    components::{Material, MeshProperties},
    Mesh3D, RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_mesh3d_indexed").memory()?;

    rec.log(
        "triangle",
        &Mesh3D::new([[0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 0.0]])
            .with_vertex_normals([[0.0, 0.0, 1.0]])
            .with_vertex_colors([0x0000FFFF, 0x00FF00FF, 0xFF0000FF])
            .with_mesh_properties(MeshProperties::from_triangle_indices([[2, 1, 0]]))
            .with_mesh_material(Material::from_albedo_factor(0xCC00CCFF)),
    )?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
