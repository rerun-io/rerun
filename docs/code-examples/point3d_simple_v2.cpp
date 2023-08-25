// Log some very simple points.

#include <rerun.hpp>

namespace rr = rerun;

int main() {
    auto rr_stream = rr::RecordingStream("rerun-example-points3d_simple");
    rr_stream.connect("127.0.0.1:9876").throw_on_failure();

    rr_stream.log("points", rr::Points3D({{0.0f, 0.0f, 0.0f}, {1.0f, 1.0f, 1.0f}}));
}
