// Use the `send_columns` API to send several point clouds over time in a single operation.

#include <array>
#include <rerun.hpp>
#include <vector>

using namespace std::chrono_literals;

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_points3d_column_updates");
    rec.spawn().exit_on_failure();

    // Prepare a point cloud that evolves over 5 timesteps, changing the number of points in the process.
    std::vector<std::array<float, 3>> positions = {
        // clang-format off
        {1.0, 0.0, 1.0}, {0.5, 0.5, 2.0},
        {1.5, -0.5, 1.5}, {1.0, 1.0, 2.5}, {-0.5, 1.5, 1.0}, {-1.5, 0.0, 2.0},
        {2.0, 0.0, 2.0}, {1.5, -1.5, 3.0}, {0.0, -2.0, 2.5}, {1.0, -1.0, 3.5},
        {-2.0, 0.0, 2.0}, {-1.5, 1.5, 3.0}, {-1.0, 1.0, 3.5},
        {1.0, -1.0, 1.0}, {2.0, -2.0, 2.0}, {3.0, -1.0, 3.0}, {2.0, 0.0, 4.0},
        // clang-format on
    };

    // At each timestep, all points in the cloud share the same but changing color and radius.
    std::vector<uint32_t> colors = {0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFF00FF, 0x00FFFFFF};
    std::vector<float> radii = {0.05f, 0.01f, 0.2f, 0.1f, 0.3f};

    // Log at seconds 10-14
    auto times = rerun::Collection{10s, 11s, 12s, 13s, 14s};
    auto time_column = rerun::TimeColumn::from_times("time", std::move(times));

    // Partition our data as expected across the 5 timesteps.
    auto position = rerun::Points3D().with_positions(positions).columns({2, 4, 4, 3, 4});
    auto color_and_radius = rerun::Points3D().with_colors(colors).with_radii(radii).columns();

    rec.send_columns("points", time_column, position, color_and_radius);
}
