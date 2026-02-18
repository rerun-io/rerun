// Log a batch of 3D arrows.

#include <rerun.hpp>

#include <cmath>
#include <vector>

constexpr float TAU = 6.28318530717958647692528676655900577f;

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_arrow3d");
    rec.spawn().exit_on_failure();

    std::vector<rerun::Position3D> origins;
    std::vector<rerun::Vector3D> vectors;
    std::vector<rerun::Color> colors;

    for (int i = 0; i < 100; ++i) {
        origins.push_back({0, 0, 0});

        float angle = TAU * static_cast<float>(i) * 0.01f;
        float length = static_cast<float>(log2(i + 1));
        vectors.push_back({length * sinf(angle), 0.0, length * cosf(angle)});

        uint8_t c = static_cast<uint8_t>(round(angle / TAU * 255.0f));
        colors.push_back({static_cast<uint8_t>(255 - c), c, 128, 128});
    }

    rec.log(
        "arrows",
        rerun::Arrows3D::from_vectors(vectors).with_origins(origins).with_colors(colors)
    );
}
