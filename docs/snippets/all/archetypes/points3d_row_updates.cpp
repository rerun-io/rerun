//! Update a point cloud over time.
//!
//! See also the `points3d_column_updates` example, which achieves the same thing in a single operation.

#include <rerun.hpp>

#include <algorithm>
#include <vector>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_points3d_row_updates");
    rec.spawn().exit_on_failure();

    // Prepare a point cloud that evolves over 5 timesteps, changing the number of points in the process.
    std::vector<std::array<float, 3>> positions[] = {
        // clang-format off
        {{1.0, 0.0, 1.0}, {0.5, 0.5, 2.0}},
        {{1.5, -0.5, 1.5}, {1.0, 1.0, 2.5}, {-0.5, 1.5, 1.0}, {-1.5, 0.0, 2.0}},
        {{2.0, 0.0, 2.0}, {1.5, -1.5, 3.0}, {0.0, -2.0, 2.5}, {1.0, -1.0, 3.5}},
        {{-2.0, 0.0, 2.0}, {-1.5, 1.5, 3.0}, {-1.0, 1.0, 3.5}},
        {{1.0, -1.0, 1.0}, {2.0, -2.0, 2.0}, {3.0, -1.0, 3.0}, {2.0, 0.0, 4.0}},
        // clang-format on
    };

    // At each timestep, all points in the cloud share the same but changing color and radius.
    std::vector<uint32_t> colors = {0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFF00FF, 0x00FFFFFF};
    std::vector<float> radii = {0.05f, 0.01f, 0.2f, 0.1f, 0.3f};

    for (size_t i = 0; i < 5; i++) {
        rec.set_time_duration_secs("time", 10.0 + static_cast<double>(i));
        rec.log(
            "points",
            rerun::Points3D(positions[i]).with_colors(colors[i]).with_radii(radii[i])
        );
    }
}
