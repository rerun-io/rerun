// Log a simple set of line segments.

#include <rerun.hpp>

#include <array>
#include <vector>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_line_segments3d");
    rec.spawn().exit_on_failure();

    std::vector<std::vector<std::array<float, 3>>> points = {
        {{0.f, 0.f, 0.f}, {0.f, 0.f, 1.f}},
        {{1.f, 0.f, 0.f}, {1.f, 0.f, 1.f}},
        {{1.f, 1.f, 0.f}, {1.f, 1.f, 1.f}},
        {{0.f, 1.f, 0.f}, {0.f, 1.f, 1.f}},
    };

    rec.log("segments", rerun::LineStrips3D(points));
}
