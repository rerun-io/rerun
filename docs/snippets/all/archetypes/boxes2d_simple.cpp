// Log some simple 2D boxes.

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_box2d");
    rec.spawn().exit_on_failure();

    rec.log("simple", rerun::Boxes2D::from_mins_and_sizes({{-1.f, -1.f}}, {{2.f, 2.f}}));
}
