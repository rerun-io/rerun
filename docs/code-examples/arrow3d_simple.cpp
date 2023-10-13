// Log a batch of 3D arrows.

#include <rerun.hpp>

#include <cmath>
#include <numeric>

#define TAU (static_cast<float>(M_PI) * 2.0f)

int main() {
    auto rec = rerun::RecordingStream("rerun_example_arrow3d");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    std::vector<rerun::components::Position3D> origins;
    std::vector<rerun::components::Vector3D> vectors;
    std::vector<rerun::components::Color> colors;

    for (int i = 0; i < 100; ++i) {
        origins.push_back({0, 0, 0});

        float angle = TAU * i * 0.01f;
        float length = log2f(i + 1);
        vectors.push_back({length * sinf(angle), 0.0, length * cosf(angle)});

        uint8_t c = static_cast<uint8_t>(round(angle / TAU * 255.0f));
        colors.push_back({static_cast<uint8_t>(255 - c), c, 128, 128});
    }

    rec.log(
        "arrows",
        rerun::Arrows3D::from_vectors(vectors).with_origins(origins).with_colors(colors)
    );
}
