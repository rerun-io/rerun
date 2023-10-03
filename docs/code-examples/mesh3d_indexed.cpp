// Log a simple colored triangle.

#include <rerun.hpp>

#include <cmath>
#include <numeric>

namespace rr = rerun;
namespace rrc = rr::components;

int main() {
    auto rec = rr::RecordingStream("rerun_example_mesh3d_indexed");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    std::vector<rr::components::Position3D> vertex_positions = {
        {0.0, 1.0, 0.0},
        {1.0, 0.0, 0.0},
        {0.0, 0.0, 0.0},
    };
    std::vector<rr::components::Color> vertex_colors = {
        {0, 0, 255},
        {0, 255, 0},
        {255, 0, 0},
    };
    std::vector<uint32_t> indices = {2, 1, 0};

    rec.log(
        "triangle",
        rr::Mesh3D(vertex_positions)
            .with_vertex_normals({0.0, 0.0, 1.0})
            .with_vertex_colors(vertex_colors)
            .with_mesh_properties(rrc::MeshProperties::from_triangle_indices(indices))
            .with_mesh_material(rrc::Material::from_albedo_factor(0xCC00CCFF))
    );
}
