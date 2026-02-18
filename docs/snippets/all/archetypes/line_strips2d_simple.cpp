// Log a simple line strip.

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_line_strip2d");
    rec.spawn().exit_on_failure();

    const auto strip = rerun::LineStrip2D({{0.f, 0.f}, {2.f, 1.f}, {4.f, -1.f}, {6.f, 0.f}});
    rec.log("strip", rerun::LineStrips2D(strip));

    // TODO(#5520): log VisualBounds2D
}
