// Log a simple line strip.

#include <rerun.hpp>

namespace rr = rerun;

int main() {
    auto rr_stream = rr::RecordingStream("line_strip2d");
    rr_stream.connect("127.0.0.1:9876");

    std::vector<rr::datatypes::Vec2D> strip = {{0.f, 0.f}, {2.f, 1.f}, {4.f, -1.f}, {6.f, 0.f}};
    rr_stream.log("strips", rr::archetypes::LineStrips2D(strip));
}
