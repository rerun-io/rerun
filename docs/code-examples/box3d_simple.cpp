// Log a single 3D box.

#include <rerun.hpp>

int main() {
    auto rec = rerun::RecordingStream("rerun_example_box3d_simple");
    rec.spawn().throw_on_failure();

    rec.log("simple", rerun::Boxes3D::from_half_sizes({{2.f, 2.f, 1.0f}}));
}
