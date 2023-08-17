// Log a simple set of line segments.

#include <rerun.hpp>

namespace rr = rerun;

int main() {
    auto rr_stream = rr::RecordingStream("line_segments3d");
    rr_stream.connect("127.0.0.1:9876").throw_on_failure();

    std::vector<rr::datatypes::Vec3D> points = {
        {0.f, 0.f, 0.f},
        {0.f, 0.f, 1.f},
        {1.f, 0.f, 0.f},
        {1.f, 0.f, 1.f},
        {1.f, 1.f, 0.f},
        {1.f, 1.f, 1.f},
        {0.f, 1.f, 0.f},
        {0.f, 1.f, 1.f},
    };
    rr_stream.log("segments", rr::archetypes::LineStrips3D(points));
}
