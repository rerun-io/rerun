// Log a couple 2D line segments using 2D line strips.

#include <rerun.hpp>

int main() {
    auto rec = rerun::RecordingStream("rerun_example_line_segments2d");
    rec.connect().throw_on_failure();

    rec.log(
        "segments",
        rerun::LineStrips2D({
            {{0.f, 0.f}, {2.f, 1.f}},
            {{4.f, -1.f}, {6.f, 0.f}},
        })
    );

    // Log an extra rect to set the view bounds
    rec.log("bounds", rerun::Boxes2D::from_centers_and_sizes({{3.0f, 0.0f}}, {{8.0f, 6.0f}}));
}
