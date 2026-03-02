// Use the math/plot convention for 2D (Y pointing up).

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_view_coordinates2d");
    rec.spawn().exit_on_failure();

    rec.log_static("world", rerun::ViewCoordinates2D::RU); // Set Y-Up

    rec.log("world/points", rerun::Points2D({{0.0, 0.0}, {1.0, 1.0}, {3.0, 2.0}}));
}
