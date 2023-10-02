// Change the view coordinates for the scene.

#include <rerun.hpp>

#include <cmath>
#include <numeric>

namespace rr = rerun;

int main() {
    auto rec = rr::RecordingStream("rerun_example_view_coordinates");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    rec.log("world", rr::ViewCoordinates::RIGHT_HAND_Z_UP); // Set an up-axis
    rec.log(
        "world/xyz",
        rr::Arrows3D::from_vectors({{1.0, 0.0, 0.0}, {0.0, 1.0, 0.0}, {0.0, 0.0, 1.0}}
        ).with_colors({{255, 0, 0}, {0, 255, 0}, {0, 0, 255}})
    );
}
