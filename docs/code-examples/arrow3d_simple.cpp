// Log a batch of 3D arrows.

#include <rerun.hpp>

#include <cmath>
#include <numeric>

namespace rr = rerun;

int main() {
    auto rec = rr::RecordingStream("rerun_example_arrow3d");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    std::vector<rr::components::Position3D> origins;
    std::vector<rr::components::Vector3D> vectors;
    std::vector<rr::components::Color> colors;

    for (int i = 0; i < 100; ++i) {
        origins.push_back({0, 0, 0});

        float angle = 2.0 * M_PI * i * 0.01f;
        float length = log2f(i + 1);
        vectors.push_back({length * sinf(angle), 0.0, length * cosf(angle)});

        uint8_t c = static_cast<uint8_t>(round(angle / (2.0 * M_PI) * 255.0));
        colors.push_back({static_cast<uint8_t>(255 - c), c, 128, 128});
    }

    rec.log(
        "arrows",
        rr::Arrows3D::from_vectors(vectors).with_origins(origins).with_colors(colors)
    );
}
