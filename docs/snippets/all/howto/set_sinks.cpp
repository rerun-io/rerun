#include <rerun.hpp>
#include <rerun/demo_utils.hpp>

using namespace rerun::demo;

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_set_sinks");
    rec.set_sinks(
           // Connect to a local viewer using the default URL.
           rerun::GrpcSink{},
           // Write data to a `data.rrd` file in the current directory.
           rerun::FileSink{"data.rrd"}
    )
        .exit_on_failure();

    // Create some data using the `grid` utility function.
    std::vector<rerun::Position3D> points = grid3d<rerun::Position3D, float>(-10.f, 10.f, 10);
    std::vector<rerun::Color> colors = grid3d<rerun::Color, uint8_t>(0, 255, 10);

    // Log the "my_points" entity with our data, using the `Points3D` archetype.
    rec.log("my_points", rerun::Points3D(points).with_colors(colors).with_radii({0.5f}));
}
