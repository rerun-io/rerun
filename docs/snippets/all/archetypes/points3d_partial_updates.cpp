//! Update specific properties of a point cloud over time.

#include <rerun.hpp>

#include <algorithm>
#include <vector>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_points3d_partial_updates");
    rec.spawn().exit_on_failure();

    std::vector<rerun::Position3D> positions;
    for (int i = 0; i < 10; ++i) {
        positions.emplace_back(static_cast<float>(i), 0.0f, 0.0f);
    }

    rec.set_time_sequence("frame", 0);
    rec.log("points", rerun::Points3D(positions));

    for (int i = 0; i < 10; ++i) {
        std::vector<rerun::Color> colors;
        for (int n = 0; n < 10; ++n) {
            if (n < i) {
                colors.emplace_back(rerun::Color(20, 200, 20));
            } else {
                colors.emplace_back(rerun::Color(200, 20, 20));
            }
        }

        std::vector<rerun::Radius> radii;
        for (int n = 0; n < 10; ++n) {
            if (n < i) {
                radii.emplace_back(rerun::Radius(0.6f));
            } else {
                radii.emplace_back(rerun::Radius(0.2f));
            }
        }

        // Update only the colors and radii, leaving everything else as-is.
        rec.set_time_sequence("frame", i);
        rec.log("points", rerun::Points3D::update_fields().with_radii(radii).with_colors(colors));
    }

    std::vector<rerun::Radius> radii;
    radii.emplace_back(0.3f);

    // Update the positions and radii, and clear everything else in the process.
    rec.set_time_sequence("frame", 20);
    rec.log("points", rerun::Points3D::clear_fields().with_positions(positions).with_radii(radii));
}
