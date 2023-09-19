// Log a simple line strip.

#include <rerun.hpp>

namespace rr = rerun;

int main() {
    auto rec = rr::RecordingStream("rerun_example_line_strip2d");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    std::vector<rr::datatypes::Vec2D> strip = {{0.f, 0.f}, {2.f, 1.f}, {4.f, -1.f}, {6.f, 0.f}};
    rec.log("strip", rr::LineStrips2D(strip));

    // Log an extra rect to set the view bounds
    rec.log("bounds", rr::Boxes2D::from_centers_and_sizes({{3.0f, 0.0f}}, {{8.0f, 6.0f}}));
}
