//! Log a simple colored triangle.

use rerun::{
    components::{Mesh3D, RawMesh3D},
    RecordingStreamBuilder,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec, storage) = RecordingStreamBuilder::new("rerun_example_mesh").memory()?;

    let mesh = RawMesh3D {
        vertex_positions: [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
            .into_iter()
            .flatten()
            .collect(),
        indices: Some([0, 1, 2].into_iter().collect()),
        vertex_normals: Some(
            [[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]]
                .into_iter()
                .flatten()
                .collect(),
        ),
        vertex_colors: Some([0xff0000ff, 0x00ff00ff, 0x0000ffff].into_iter().collect()),
        albedo_factor: None,
    };

    // TODO(#2788): Mesh archetype
    rec.log_component_lists("triangle", false, 1, [&Mesh3D::Raw(mesh) as _])?;

    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
