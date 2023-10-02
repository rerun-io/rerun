// Log a simple line strip.

#include <rerun.hpp>

namespace rr = rerun;

int main() {
    auto rec = rr::RecordingStream("rerun_example_line_strip3d");
    rec.connect("127.0.0.1:9876").throw_on_failure();

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
    rec.log("strip", rr::LineStrips3D(points));
}
