// Log a couple 2D line segments using 2D line strips.

#include <rerun.hpp>

namespace rr = rerun;

int main() {
    auto rec = rr::RecordingStream("rerun_example_line_segments2d");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    // TODO(#3202): I want to do this!
    // std::vector<std::vector<rr::datatypes::Vec2D>> points = {
    //     {{0.f, 0.f}, {2.f, 1.f}},
    //     {{4.f, -1.f}, {6.f, 0.f}},
    // };
    // rec.log("segments", rr::LineStrips2D(points));

    std::vector<rr::datatypes::Vec2D> points1 = {{0.f, 0.f}, {2.f, 1.f}};
    std::vector<rr::datatypes::Vec2D> points2 = {{4.f, -1.f}, {6.f, 0.f}};
    rec.log("segments", rr::LineStrips2D({points1, points2}));

    // TODO(#2786): Rect2D archetype
}
