// Log some very simple points.

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_points3d");
    rec.spawn().exit_on_failure();

    rec.log("points", rerun::Points3D({{0.0f, 0.0f, 0.0f}, {1.0f, 1.0f, 1.0f}}));
}
