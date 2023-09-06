// Log a batch of 2d line strips.

#include <rerun.hpp>

namespace rr = rerun;

int main() {
    auto rec = rr::RecordingStream("rerun_example_line_strip2d");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    std::vector<rr::datatypes::Vec2D> strip1 = {{0.f, 0.f}, {2.f, 1.f}, {4.f, -1.f}, {6.f, 0.f}};
    std::vector<rr::datatypes::Vec2D> strip2 =
        {{0.f, 3.f}, {1.f, 4.f}, {2.f, 2.f}, {3.f, 4.f}, {4.f, 2.f}, {5.f, 4.f}, {6.f, 3.f}};
    rec.log(
        "strips",
        rr::LineStrips2D({strip1, strip2})
            .with_colors({0xFF0000FF, 0x00FF00FF})
            .with_radii({0.025f, 0.005f})
            .with_labels({"one strip here", "and one strip there"})
    );

    // TODO(#2786): Rect2D archetype
}
