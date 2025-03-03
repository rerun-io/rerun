//! Log a simple 3D mesh with several instance pose transforms which instantiate the mesh several times and will not affect its children (known as mesh instancing).

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rec = rerun::RecordingStreamBuilder::new("rerun_example_mesh3d_instancing").spawn()?;

    rec.set_index("frame", sequence=0);
    rec.log(
        "shape",
        &rerun::Mesh3D::new([
            [1.0, 1.0, 1.0],
            [-1.0, -1.0, 1.0],
            [-1.0, 1.0, -1.0],
            [1.0, -1.0, -1.0],
        ])
        .with_triangle_indices([[0, 1, 2], [0, 1, 3], [0, 2, 3], [1, 2, 3]])
        .with_vertex_colors([0xFF0000FF, 0x00FF00FF, 0x00000FFFF, 0xFFFF00FF]),
    )?;
    // This box will not be affected by its parent's instance poses!
    rec.log(
        "shape/box",
        &rerun::Boxes3D::from_half_sizes([[5.0, 5.0, 5.0]]),
    )?;

    for i in 0..100 {
        rec.set_index("frame", sequence=i);
        rec.log(
            "shape",
            &rerun::InstancePoses3D::new()
                .with_translations([
                    [2.0, 0.0, 0.0],
                    [0.0, 2.0, 0.0],
                    [0.0, -2.0, 0.0],
                    [-2.0, 0.0, 0.0],
                ])
                .with_rotation_axis_angles([rerun::RotationAxisAngle::new(
                    [0.0, 0.0, 1.0],
                    rerun::Angle::from_degrees(i as f32 * 2.0),
                )]),
        )?;
    }

    Ok(())
}
