// Log a simple line strip.

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_line_strip3d");
    rec.spawn().exit_on_failure();

    rerun::LineStrip3D linestrip({
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
