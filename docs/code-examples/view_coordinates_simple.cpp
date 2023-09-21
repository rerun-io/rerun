// Log a batch of 3D arrows.

#include <rerun.hpp>

#include <cmath>
#include <numeric>

namespace rr = rerun;

int main() {
    auto rec = rr::RecordingStream("rerun_example_view_coordinate");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    rec.log("/", rr::archetypes::ViewCoordinates::ULB);
    rec.log(
        "xyz",
        rr::Arrows3D({{1.0, 0.0, 0.0}, {0.0, 1.0, 0.0}, {0.0, 0.0, 1.0}}
        ).with_colors({{255, 0, 0}, {0, 255, 0}, {0, 0, 255}})
    );
}
