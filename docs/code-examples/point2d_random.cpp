// Log some very simple points.

#include <rerun.hpp>

#include <algorithm>
#include <random>

int main() {
    auto rec = rerun::RecordingStream("rerun_example_points2d_simple");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    std::default_random_engine gen;
    std::uniform_real_distribution<float> dist_pos(-5.0f, 5.0f);
    std::uniform_real_distribution<float> dist_radius(0.1f, 1.0f);
    std::uniform_int_distribution<uint8_t> dist_color(0, 255);

    std::vector<rerun::components::Position2D> points2d(10);
    std::generate(points2d.begin(), points2d.end(), [&] {
        return rerun::components::Position2D(dist_pos(gen), dist_pos(gen));
    });
    std::vector<rerun::components::Color> colors(10);
    std::generate(colors.begin(), colors.end(), [&] {
        return rerun::components::Color(dist_color(gen), dist_color(gen), dist_color(gen));
    });
    std::vector<rerun::components::Radius> radii(10);
    std::generate(radii.begin(), radii.end(), [&] { return dist_radius(gen); });

    rec.log("random", rerun::Points2D(points2d).with_colors(colors).with_radii(radii));

    // Log an extra rect to set the view bounds
    rec.log("bounds", rerun::Boxes2D::from_half_sizes({{2.0f, 1.5f}}));
}
