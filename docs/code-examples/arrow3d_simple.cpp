// Log a batch of 3D arrows.

#include <rerun.hpp>

#include <cmath>
#include <numeric>

namespace rr = rerun;

int main() {
    auto rec = rr::RecordingStream("rerun_example_arrow3d");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    std::vector<rr::components::Vector3D> vectors;
    std::vector<rr::components::Color> colors;

    for (int i = 0; i < 100; ++i) {
        double angle = 2.0 * M_PI * i * 0.01f;
        double length = log2f(i + 1);
        vectors.push_back({length * sin(angle), 0.0, length * cos(angle)});

        uint8_t c = static_cast<uint8_t>(round(angle / (2.0 * M_PI) * 255.0));
        colors.push_back({static_cast<uint8_t>(255 - c), c, 128, 128});
    }

    rec.log("arrows", rr::Arrows3D(vectors).with_colors(colors));
}
