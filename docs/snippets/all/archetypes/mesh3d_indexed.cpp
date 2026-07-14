// Log a simple colored triangle.

#include <rerun.hpp>

#include <vector>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_mesh3d_indexed");
    rec.spawn().exit_on_failure();

    const rerun::Position3D vertex_positions[3] = {
        {0.0f, 1.0f, 0.0f},
        {1.0f, 0.0f, 0.0f},
        {0.0f, 0.0f, 0.0f},
    };
    const rerun::Color vertex_colors[3] = {
        {0, 0, 255},
        {0, 255, 0},
        {255, 0, 0},
    };

    rec.log(
        "triangle",
        rerun::Mesh3D(vertex_positions)
            .with_vertex_normals({{0.0, 0.0, 1.0}})
            .with_vertex_colors(vertex_colors)
            .with_triangle_indices({{2, 1, 0}})
    );
}
