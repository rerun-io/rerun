// Log a batch of 3D arrows.

#include <rerun.hpp>

#include <cmath>
#include <numeric>

namespace rr = rerun;

int main() {
    auto rr_stream = rr::RecordingStream("arrow3d");
    rr_stream.connect("127.0.0.1:9876");

    std::vector<rr::components::Vector3D> vectors;
    std::vector<rr::components::Color> colors;

    for (int i = 0; i < 100; ++i) {
        float angle = 2.0 * M_PI * i * 0.01f;
        float length = log2f(i + 1);
        vectors.push_back(rr::datatypes::Vec3D{length * sinf(angle), 0.0, length * cosf(angle)});

        // TODO(andreas): provide `unmultiplied_rgba`
        uint8_t c = static_cast<uint8_t>((angle / (2.0 * M_PI) * 255.0) + 0.5);
        uint32_t color = ((255 - c) << 24) + (c << 16) + (128 << 8) + (128 << 0);
        colors.push_back(color);
    }

    rr_stream.log("arrows", rr::archetypes::Arrows3D(vectors).with_colors(colors));
}
