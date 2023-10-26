// Log some random points with color and radii.

#include <rerun.hpp>

#include <algorithm>
#include <random>

int main() {
    auto rec = rerun::RecordingStream("rerun_example_points3d_random");
    rec.spawn().throw_on_failure();

    std::default_random_engine gen;
    std::uniform_real_distribution<float> dist_pos(-5.0f, 5.0f);
    std::uniform_real_distribution<float> dist_radius(0.1f, 1.0f);
    // On MSVC uint8_t distributions are not supported.
    std::uniform_int_distribution<int> dist_color(0, 255);

    std::vector<rerun::components::Position3D> points3d(10);
    std::generate(points3d.begin(), points3d.end(), [&] {
        return rerun::components::Position3D(dist_pos(gen), dist_pos(gen), dist_pos(gen));
    });
    std::vector<rerun::components::Color> colors(10);
    std::generate(colors.begin(), colors.end(), [&] {
        return rerun::components::Color(
            static_cast<uint8_t>(dist_color(gen)),
            static_cast<uint8_t>(dist_color(gen)),
            static_cast<uint8_t>(dist_color(gen))
        );
    });
    std::vector<rerun::components::Radius> radii(10);
    std::generate(radii.begin(), radii.end(), [&] { return dist_radius(gen); });

    rec.log("random", rerun::Points3D(points3d).with_colors(colors).with_radii(radii));
}
