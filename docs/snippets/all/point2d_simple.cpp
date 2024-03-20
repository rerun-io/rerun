// Log some very simple points.

#include <rerun.hpp>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_points2d");
    rec.spawn().exit_on_failure();

    rec.log("points", rerun::Points2D({{0.0f, 0.0f}, {1.0f, 1.0f}}));

    // Log an extra rect to set the view bounds
    rec.log("bounds", rerun::Boxes2D::from_half_sizes({{2.0f, 1.5f}}));
}
