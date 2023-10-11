// Log a simple colored triangle.

#include <rerun.hpp>

#include <cmath>
#include <numeric>

namespace rr = rerun;

int main() {
    auto rec = rr::RecordingStream("rerun_example_mesh3d_simple");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    std::vector<rr::components::Position3D> vertex_positions = {
        {0.0, 0.0, 0.0},
        {1.0, 0.0, 0.0},
        {0.0, 1.0, 0.0},
    };
    std::vector<rr::components::Color> vertex_colors = {
        {255, 0, 0},
        {0, 255, 0},
        {0, 0, 255},
    };

    rec.log(
        "triangle",
        rr::Mesh3D(vertex_positions)
            .with_vertex_normals({{0.0f, 0.0f, 1.0f}})
            .with_vertex_colors(vertex_colors)
    );
}
