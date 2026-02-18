// Log a batch of 2D arrows.

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_arrow2d");
    rec.spawn().exit_on_failure();

    rec.log(
        "arrows",
        rerun::Arrows2D::from_vectors({{1.0f, 0.0f}, {0.0f, -1.0f}, {-0.7f, 0.7f}})
            .with_radii(0.025f)
            .with_origins({{0.25f, 0.0f}, {0.25f, 0.0f}, {-0.1f, -0.1f}})
            .with_colors({{255, 0, 0}, {0, 255, 0}, {127, 0, 255}})
            .with_labels({"right", "up", "left-down"})
    );
}
