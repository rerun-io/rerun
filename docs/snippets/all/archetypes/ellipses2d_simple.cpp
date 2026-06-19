// Log some simple 2D ellipses.

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_ellipses2d");
    rec.spawn().exit_on_failure();

    rec.log(
        "simple",
        rerun::Ellipses2D::from_centers_and_half_sizes(
            {{0.0f, 0.0f}},
            {{2.0f, 1.0f}}
        )
    );
}
