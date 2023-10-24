#include <cmath>

#include <rerun.hpp>

float remap(float value, float from_min, float from_max, float to_min, float to_max) {
    return to_min + (to_max - to_min) * (value - from_min) / (from_max - from_min);
}

int main() {
    // Create a cube of colored points:

    std::vector<rerun::Position3D> positions;
    std::vector<rerun::Color> colors;
    positions.reserve(10 * 10 * 10);
    colors.reserve(10 * 10 * 10);

    for (float zi = 0.f; zi < 10.f; zi += 1.f) {
        for (float yi = 0.f; yi < 10.f; yi += 1.f) {
            for (float xi = 0.f; xi < 10.f; xi += 1.f) {
                const float xf = remap(xi, 0.f, 10.f, -10.f, 10.f);
                const float yf = remap(yi, 0.f, 10.f, -10.f, 10.f);
                const float zf = remap(zi, 0.f, 10.f, -10.f, 10.f);

                positions.emplace_back(xf, yf, zf);

                const auto r = static_cast<uint8_t>(roundf(remap(xi, 0.f, 10.f, 0.f, 255.f)));
                const auto g = static_cast<uint8_t>(roundf(remap(yi, 0.f, 10.f, 0.f, 255.f)));
                const auto b = static_cast<uint8_t>(roundf(remap(zi, 0.f, 10.f, 0.f, 255.f)));

                colors.emplace_back(r, g, b);
            }
        }
    }

    auto rec = rerun::RecordingStream("rerun_example_demo");
    rec.connect().throw_on_failure();
    rec.log("points", rerun::Points3D(positions).with_colors(colors).with_radii({0.5}));
}
