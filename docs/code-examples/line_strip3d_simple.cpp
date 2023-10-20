// Log a simple line strip.

#include <rerun.hpp>

int main() {
    auto rec = rerun::RecordingStream("rerun_example_line_strip3d");
    rec.connect().throw_on_failure();

    rerun::components::LineStrip3D linestrip({
        {0.f, 0.f, 0.f},
        {0.f, 0.f, 1.f},
        {1.f, 0.f, 0.f},
        {1.f, 0.f, 1.f},
        {1.f, 1.f, 0.f},
        {1.f, 1.f, 1.f},
        {0.f, 1.f, 0.f},
        {0.f, 1.f, 1.f},
    });
    rec.log("strip", rerun::LineStrips3D(linestrip));
}
