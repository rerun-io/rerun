// Log a simple colored triangle, then update its vertices' positions each frame.

#include <rerun.hpp>

#include <cmath>
#include <numeric>

rerun::Position3D mul_pos(float factor, rerun::Position3D vec) {
    return {factor * vec.x(), factor * vec.y(), factor * vec.z()};
}

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_mesh3d_partial_updates");
    rec.spawn().exit_on_failure();

    rerun::Position3D vertex_positions[3] = {
        {-1.0f, 0.0f, 0.0f},
        {1.0f, 0.0f, 0.0f},
        {0.0f, 1.0f, 0.0f},
    };
    rerun::Color vertex_colors[3] = {
        {255, 0, 0},
        {0, 255, 0},
        {0, 0, 255},
    };

    // Log the initial state of our triangle:
    rec.set_time_sequence("frame", 0);
    rec.log(
        "triangle",
        rerun::Mesh3D(vertex_positions)
            .with_vertex_normals({{0.0f, 0.0f, 1.0f}})
            .with_vertex_colors(vertex_colors)
    );

    // Only update its vertices' positions each frame
    for (int i = 1; i < 300; ++i) {
        rec.set_time_sequence("frame", i);

        const auto factor = fabsf(sinf(static_cast<float>(i) * 0.04f));
        const auto modified_vertex_positions = {
            mul_pos(factor, vertex_positions[0]),
            mul_pos(factor, vertex_positions[1]),
            mul_pos(factor, vertex_positions[2]),
        };
        rec.log(
            "triangle",
            rerun::Mesh3D::update_fields().with_vertex_positions(modified_vertex_positions)
        );
    }
}
