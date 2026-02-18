// Log some random points with color and radii.

#include <rerun.hpp>

#include <algorithm>
#include <random>
#include <vector>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_points2d_random");
    rec.spawn().exit_on_failure();

    std::default_random_engine gen;
    std::uniform_real_distribution<float> dist_pos(-3.0f, 3.0f);
    std::uniform_real_distribution<float> dist_radius(0.1f, 1.0f);
    // On MSVC uint8_t distributions are not supported.
    std::uniform_int_distribution<int> dist_color(0, 255);

    std::vector<rerun::Position2D> points2d(10);
    std::generate(points2d.begin(), points2d.end(), [&] {
        return rerun::Position2D(dist_pos(gen), dist_pos(gen));
    });
    std::vector<rerun::Color> colors(10);
    std::generate(colors.begin(), colors.end(), [&] {
        return rerun::Color(
            static_cast<uint8_t>(dist_color(gen)),
            static_cast<uint8_t>(dist_color(gen)),
            static_cast<uint8_t>(dist_color(gen))
        );
    });
    std::vector<rerun::Radius> radii(10);
    std::generate(radii.begin(), radii.end(), [&] { return dist_radius(gen); });

    rec.log("random", rerun::Points2D(points2d).with_colors(colors).with_radii(radii));

    // TODO(#5520): log VisualBounds2D
}
