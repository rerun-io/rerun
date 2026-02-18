// Log a simple 3D mesh with several instance pose transforms which instantiate the mesh several times and will not affect its children (known as mesh instancing).

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_mesh3d_instancing");
    rec.spawn().exit_on_failure();

    rec.set_time_sequence("frame", 0);
    rec.log(
        "shape",
        rerun::Mesh3D(
            {{1.0f, 1.0f, 1.0f}, {-1.0f, -1.0f, 1.0f}, {-1.0f, 1.0f, -1.0f}, {1.0f, -1.0f, -1.0f}}
        )
            .with_triangle_indices({{0, 2, 1}, {0, 3, 1}, {0, 3, 2}, {1, 3, 2}})
            .with_vertex_colors({0xFF0000FF, 0x00FF00FF, 0x00000FFFF, 0xFFFF00FF})
    );
    // This box will not be affected by its parent's instance poses!
    rec.log("shape/box", rerun::Boxes3D::from_half_sizes({{5.0f, 5.0f, 5.0f}}));

    for (int i = 0; i < 100; ++i) {
        rec.set_time_sequence("frame", i);
        rec.log(
            "shape",
            rerun::InstancePoses3D()
                .with_translations(
                    {{2.0f, 0.0f, 0.0f},
                     {0.0f, 2.0f, 0.0f},
                     {0.0f, -2.0f, 0.0f},
                     {-2.0f, 0.0f, 0.0f}}
                )
                .with_rotation_axis_angles({rerun::RotationAxisAngle(
                    {0.0f, 0.0f, 1.0f},
                    rerun::Angle::degrees(static_cast<float>(i) * 2.0f)
                )})
        );
    }
}
