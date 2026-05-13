// Log a batch of 2D ellipses.

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_ellipses2d_batch");
    rec.spawn().exit_on_failure();

    rec.log(
        "batch",
        rerun::Ellipses2D::from_centers_and_half_sizes(
            {{-2.0f, 0.0f}, {0.0f, 0.0f}, {2.5f, 0.0f}},
            {{1.5f, 0.75f}, {0.5f, 0.5f}, {0.75f, 1.5f}}
        )
            .with_line_radii({0.025f, 0.05f, 0.025f})
            .with_colors({
                rerun::Rgba32(255, 0, 0),
                rerun::Rgba32(0, 255, 0),
                rerun::Rgba32(0, 0, 255),
            })
            .with_labels({"wide", "circle", "tall"})
    );
}
