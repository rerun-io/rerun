// Log a batch of 3D arrows.

#include <rerun.hpp>

#include <cmath>
#include <numeric>

namespace rr = rerun;

int main() {
    auto rr_stream = rr::RecordingStream("rerun_example_arrow3d");
    rr_stream.connect("127.0.0.1:9876").throw_on_failure();

    std::vector<rr::components::Vector3D> vectors;
    std::vector<rr::components::Color> colors;

    for (int i = 0; i < 100; ++i) {
        float angle = 2.0 * M_PI * i * 0.01f;
        float length = log2f(i + 1);
        vectors.push_back({length * sinf(angle), 0.0, length * cosf(angle)});

        uint8_t c = static_cast<uint8_t>(angle / (2.0 * M_PI) * 255.0);
        colors.push_back({static_cast<uint8_t>(255 - c), c, 128, 128});
    }

    rr_stream.log("arrows", rr::Arrows3D(vectors).with_colors(colors));
}
