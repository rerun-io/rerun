// Log some random points with color and radii.

#include <rerun.hpp>

#include <algorithm>
#include <random>

namespace rr = rerun;

int main() {
    auto rec = rr::RecordingStream("rerun_example_points3d_random");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    std::default_random_engine gen;
    std::uniform_real_distribution<float> dist_pos(-5.0, 5.0);
    std::uniform_real_distribution<float> dist_radius(0.1, 1.0);
    std::uniform_int_distribution<uint8_t> dist_color(0, 255);

    std::vector<rr::components::Point3D> points3d(10);
    std::generate(points3d.begin(), points3d.end(), [&] {
        return rr::components::Point3D(dist_pos(gen), dist_pos(gen), dist_pos(gen));
    });
    std::vector<rr::components::Color> colors(10);
    std::generate(colors.begin(), colors.end(), [&] {
        return rr::components::Color(dist_color(gen), dist_color(gen), dist_color(gen));
    });
    std::vector<rr::components::Radius> radii(10);
    std::generate(radii.begin(), radii.end(), [&] { return dist_radius(gen); });

    rec.log("random", rr::Points3D(points3d).with_colors(colors).with_radii(radii));
}
