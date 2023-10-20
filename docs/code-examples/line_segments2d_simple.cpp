// Log a couple 2D line segments using 2D line strips.

#include <rerun.hpp>

int main() {
    auto rec = rerun::RecordingStream("rerun_example_line_segments2d");
    rec.connect().throw_on_failure();

    // TODO(#3202): I want to do this!
    // std::vector<std::vector<rerun::datatypes::Vec2D>> points = {
    //     {{0.f, 0.f}, {2.f, 1.f}},
    //     {{4.f, -1.f}, {6.f, 0.f}},
    // };
    // rec.log("segments", rerun::LineStrips2D(points));

    std::vector<rerun::datatypes::Vec2D> points1 = {{0.f, 0.f}, {2.f, 1.f}};
    std::vector<rerun::datatypes::Vec2D> points2 = {{4.f, -1.f}, {6.f, 0.f}};
    rec.log("segments", rerun::LineStrips2D({points1, points2}));

    // Log an extra rect to set the view bounds
    rec.log("bounds", rerun::Boxes2D::from_centers_and_sizes({{3.0f, 0.0f}}, {{8.0f, 6.0f}}));
}
