// Log some very simple points.

#include <rerun.hpp>

namespace rr = rerun;

int main() {
    auto rr_stream = rr::RecordingStream("points3d_simple");
    rr_stream.connect("127.0.0.1:9876");

    rr_stream.log(
        "points",
        rr::archetypes::Points3D(
            {rr::datatypes::Vec3D{0.0f, 0.0f, 0.0f}, rr::datatypes::Vec3D{1.0f, 1.0f, 1.0f}}
        )
    );
}
