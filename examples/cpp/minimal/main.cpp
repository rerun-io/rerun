#include <rerun.hpp>
#include <rerun/demo_utils.hpp>

using rerun::demo::grid;

int main() {
    // Create a new `RecordingStream` which sends data over TCP to the viewer process.
    auto rec = rerun::RecordingStream("rerun_example_cpp");
    // Try to spawn a new viewer instance.
    rec.spawn().throw_on_failure();

    // Create some data using the `grid` utility function.
    auto points = grid<rerun::Position3D, float>({-10.f, -10.f, -10.f}, {10.f, 10.f, 10.f}, 10);
    auto colors = grid<rerun::Color, uint8_t>({0, 0, 0}, {255, 255, 255}, 10);

    // Log the "my_points" entity with our data, using the `Points3D` archetype.
    rec.log("my_points", rerun::Points3D(points).with_colors(colors).with_radii({0.5f}));
}
