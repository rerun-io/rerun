// Log a batch of 2D line strips.

#include <rerun.hpp>

#include <vector>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_line_strip2d_batch");
    rec.spawn().exit_on_failure();

    rerun::Collection<rerun::Vec2D> strip1 = {{0.f, 0.f}, {2.f, 1.f}, {4.f, -1.f}, {6.f, 0.f}};
    rerun::Collection<rerun::Vec2D> strip2 =
        {{0.f, 3.f}, {1.f, 4.f}, {2.f, 2.f}, {3.f, 4.f}, {4.f, 2.f}, {5.f, 4.f}, {6.f, 3.f}};
    rec.log(
        "strips",
        rerun::LineStrips2D({strip1, strip2})
            .with_colors({0xFF0000FF, 0x00FF00FF})
            .with_radii({0.025f, 0.005f})
            .with_labels({"one strip here", "and one strip there"})
    );

    // TODO(#5520): log VisualBounds2D
}
