// Use the `send_columns` API to send several point clouds over time in a single call.

#include <array>
#include <rerun.hpp>
#include <vector>

using namespace std::chrono_literals;

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_send_columns_arrays");
    rec.spawn().exit_on_failure();

    // Prepare a point cloud that evolves over time 5 timesteps, changing the number of points in the process.
    std::vector<std::array<float, 3>> positions = {
        // clang-format off
        {1.0, 0.0, 1.0}, {0.5, 0.5, 2.0},
        {1.5, -0.5, 1.5}, {1.0, 1.0, 2.5}, {-0.5, 1.5, 1.0}, {-1.5, 0.0, 2.0},
        {2.0, 0.0, 2.0}, {1.5, -1.5, 3.0}, {0.0, -2.0, 2.5}, {1.0, -1.0, 3.5},
        {-2.0, 0.0, 2.0}, {-1.5, 1.5, 3.0}, {-1.0, 1.0, 3.5},
        {1.0, -1.0, 1.0}, {2.0, -2.0, 2.0}, {3.0, -1.0, 3.0}, {2.0, 0.0, 4.0},
        // clang-format on
    };

    // At each time stamp, all points in the cloud share the same but changing color.
    std::vector<uint32_t> colors = {0xFF0000FF, 0x00FF00FF, 0x0000FFFF, 0xFFFF00FF, 0x00FFFFFF};

    // Log at seconds 10-14
    auto times = rerun::Collection{10s, 11s, 12s, 13s, 14s};
    auto time_column = rerun::TimeColumn::from_times("time", std::move(times));

    // Interpret raw positions and color data as rerun components and partition them.
    auto indicator_batch = rerun::ComponentColumn::from_indicators<rerun::Points3D>(5);
    auto position_batch = rerun::ComponentColumn::from_loggable_with_lengths(
        rerun::Collection<rerun::components::Position3D>(std::move(positions)),
        {2, 4, 4, 3, 4}
    );
    auto color_batch = rerun::ComponentColumn::from_loggable(
        rerun::Collection<rerun::components::Color>(std::move(colors))
    );

    rec.send_columns(
        "points",
        time_column,
        {
            indicator_batch.value_or_throw(),
            position_batch.value_or_throw(),
            color_batch.value_or_throw(),
        }
    );
}
