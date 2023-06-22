//! Log a simple colored triangle.
use rerun::components::{Mesh3D, MeshId, RawMesh3D};
use rerun::{MsgSender, RecordingStreamBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (rec_stream, storage) = RecordingStreamBuilder::new("mesh").memory()?;

    let mesh = RawMesh3D {
        mesh_id: MeshId::random(),
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
        vertex_colors: Some([0xff000000, 0x00ff0000, 0x0000ff00].into_iter().collect()),
        albedo_factor: None,
    };

    MsgSender::new("triangle")
        .with_component(&[Mesh3D::Raw(mesh)])?
        .send(&rec_stream)?;

    rec_stream.flush_blocking();
    rerun::native_viewer::show(storage.take())?;
    Ok(())
}
