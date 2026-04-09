// Log a simple occupancy grid map.

#include <rerun.hpp>

#include <vector>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_grid_map");
    rec.spawn().exit_on_failure();

    const size_t width = 64;
    const size_t height = 64;
    const float cell_size = 0.1f;

    // Create a synthetic image with ROS `nav_msgs/OccupancyGrid` cell value conventions:
    // -1 (255) unknown, 0 free, 100 occupied.
    std::vector<uint8_t> grid(width * height, 255);
    for (size_t y = 8; y < 56; ++y) {
        for (size_t x = 8; x < 56; ++x) {
            grid[y * width + x] = 0;
        }
    }
    for (size_t y = 20; y < 44; ++y) {
        for (size_t x = 20; x < 44; ++x) {
            grid[y * width + x] = 100;
        }
    }

    rec.log(
        "world/map",
        rerun::archetypes::GridMap()
            .with_data(rerun::components::ImageBuffer(grid))
            .with_format(rerun::components::ImageFormat(
                {width, height},
                rerun::ColorModel::L,
                rerun::ChannelDatatype::U8
            ))
            .with_cell_size(cell_size)
            .with_translation(
                {-(static_cast<float>(width) * cell_size) / 2.0f,
                 -(static_cast<float>(height) * cell_size) / 2.0f,
                 0.0f}
            )
    );
}
