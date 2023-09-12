// Log some simple 2D boxes.

#include <rerun.hpp>

namespace rr = rerun;

int main() {
    auto rec = rr::RecordingStream("rerun_example_rect2d");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    rec.log("simple", rr::Boxes2D::from_mins_and_sizes({{-1.f, -1.f}}, {{2.f, 2.f}}));

    // Log an extra rect to set the view bounds
    rec.log("bounds", rr::Boxes2D({2.f, 1.5f}));
}
