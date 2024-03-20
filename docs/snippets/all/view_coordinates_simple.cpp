// Change the view coordinates for the scene.

#include <rerun.hpp>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_view_coordinates");
    rec.spawn().exit_on_failure();

    rec.log_timeless("world", rerun::ViewCoordinates::RIGHT_HAND_Z_UP); // Set an up-axis
    rec.log(
        "world/xyz",
        rerun::Arrows3D::from_vectors({{1.0, 0.0, 0.0}, {0.0, 1.0, 0.0}, {0.0, 0.0, 1.0}}
        ).with_colors({{255, 0, 0}, {0, 255, 0}, {0, 0, 255}})
    );
}
