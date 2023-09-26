// Log a batch of 3D arrows.

#include <rerun.hpp>

#include <cmath>
#include <numeric>

namespace rr = rerun;

int main() {
    auto rec = rr::RecordingStream("rerun_example_clear_recursive");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    std::vector<rr::components::Vector3D> vectors = {
        {1.0, 0.0, 0.0},
        {0.0, -1.0, 0.0},
        {-1.0, 0.0, 0.0},
        {0.0, 1.0, 0.0},
    };
    std::vector<rr::components::Origin3D> origins = {
        {-0.5, 0.5, 0.0},
        {0.5, 0.5, 0.0},
        {0.5, -0.5, 0.0},
        {-0.5, -0.5, 0.0},
    };
    std::vector<rr::components::Color> colors = {
        {200, 0, 0},
        {0, 200, 0},
        {0, 0, 200},
        {200, 0, 200}};

    // Log a handful of arrows.
    for (int i = 0; i < vectors.size(); ++i) {
        auto entity_path = "arrows/" + std::to_string(i);
        rec.log(
            entity_path.c_str(),
            rr::Arrows3D::from_vectors(vectors[i]).with_origins(origins[i]).with_colors(colors[i])
        );
    }

    // Now clear all of them at once.
    rec.log("arrows", rr::Clear::RECURSIVE);
}
