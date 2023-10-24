// Log a simple set of line segments.

#include <rerun.hpp>

int main() {
    auto rec = rerun::RecordingStream("rerun_example_line_segments3d");
    rec.connect().throw_on_failure();

    std::vector<std::vector<std::array<float, 3>>> points = {
        {{0.f, 0.f, 0.f}, {0.f, 0.f, 1.f}},
        {{1.f, 0.f, 0.f}, {1.f, 0.f, 1.f}},
        {{1.f, 1.f, 0.f}, {1.f, 1.f, 1.f}},
        {{0.f, 1.f, 0.f}, {0.f, 1.f, 1.f}},
    };

    rec.log("segments", rerun::LineStrips3D(points));
}
