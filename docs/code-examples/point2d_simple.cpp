// Log some very simple points.

#include <rerun.hpp>

namespace rr = rerun;

int main() {
    auto rec = rr::RecordingStream("rerun_example_points2d_simple");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    rec.log("points", rr::Points2D({{0.0f, 0.0f}, {1.0f, 1.0f}}));

    // TODO(#2786): Rect2D archetype
}
