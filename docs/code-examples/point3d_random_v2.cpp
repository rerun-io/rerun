// Log some random points with color and radii.

#include <rerun.hpp>

#include <random>

namespace rr = rerun;

int main() {
    auto rr_stream = rr::RecordingStream("points3d_random");
    rr_stream.connect("127.0.0.1:9876");

    std::default_random_engine gen;
    std::uniform_real_distribution<float> dist_pos(-5.0, 5.0);
    std::uniform_real_distribution<float> dist_radius(0.1, 1.0);
    std::uniform_int_distribution<uint8_t> dist_color(0, 255);

    std::vector<rr::components::Point3D> points3d(10);
    std::generate(points3d.begin(), points3d.end(), [&] {
        return rr::datatypes::Vec3D{dist_pos(gen), dist_pos(gen), dist_pos(gen)};
    });
    std::vector<rr::components::Color> colors(10);
    std::generate(colors.begin(), colors.end(), [&] {
        // TODO(andreas): provide a `rgb` factory method.
        return (dist_color(gen) << 24) + (dist_color(gen) << 16) + (dist_color(gen) << 8);
    });
    std::vector<rr::components::Radius> radii(10);
    std::generate(radii.begin(), radii.end(), [&] { return dist_radius(gen); });

    rr_stream.log(
        "random",
        rr::archetypes::Points3D(points3d).with_colors(colors).with_radii(radii)
    );
}
