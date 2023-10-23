// Log a simple set of line segments.

#include <rerun.hpp>

int main() {
    auto rec = rerun::RecordingStream("rerun_example_line_segments3d");
    rec.connect().throw_on_failure();

    rec.log(
        "segments",
        rerun::LineStrips3D({
            {{0.f, 0.f, 0.f}, {0.f, 0.f, 1.f}},
            {{1.f, 0.f, 0.f}, {1.f, 0.f, 1.f}},
            {{1.f, 1.f, 0.f}, {1.f, 1.f, 1.f}},
            {{0.f, 1.f, 0.f}, {0.f, 1.f, 1.f}},
        })
    );
}
