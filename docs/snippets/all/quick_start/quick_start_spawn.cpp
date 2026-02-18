#include <rerun.hpp>
#include <rerun/demo_utils.hpp>

using namespace rerun::demo;

int main(int argc, char* argv[]) {
    // Create a new `RecordingStream` which sends data over gRPC to the viewer process.
    const auto rec = rerun::RecordingStream("rerun_example_quick_start_spawn");
    rec.spawn().exit_on_failure();

    // Create some data using the `grid` utility function.
    std::vector<rerun::Position3D> points = grid3d<rerun::Position3D, float>(-10.f, 10.f, 10);
    std::vector<rerun::Color> colors = grid3d<rerun::Color, uint8_t>(0, 255, 10);

    // Log the "my_points" entity with our data, using the `Points3D` archetype.
    rec.log("my_points", rerun::Points3D(points).with_colors(colors).with_radii({0.5f}));
}
