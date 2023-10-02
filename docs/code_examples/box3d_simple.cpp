// Log a single 3D box.

#include <rerun.hpp>

namespace rr = rerun;

int main() {
    auto rec = rr::RecordingStream("rerun_example_box3d_simple");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    rec.log("simple", rr::Boxes3D::from_half_sizes({{2.f, 2.f, 1.0f}}));
}
