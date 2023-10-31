// Log a simple line strip.

#include <rerun.hpp>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_line_strip2d");
    rec.spawn().exit_on_failure();

    const auto strip = rerun::LineStrip2D({{0.f, 0.f}, {2.f, 1.f}, {4.f, -1.f}, {6.f, 0.f}});
    rec.log("strip", rerun::LineStrips2D(strip));

    // Log an extra rect to set the view bounds
    rec.log("bounds", rerun::Boxes2D::from_centers_and_sizes({{3.0f, 0.0f}}, {{8.0f, 6.0f}}));
}
