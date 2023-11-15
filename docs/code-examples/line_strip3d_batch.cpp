// Log a batch of 3d line strips.

#include <rerun.hpp>

#include <vector>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_line_strip3d");
    rec.spawn().exit_on_failure();

    std::vector<rerun::Vec3D> strip1 = {
        {0.f, 0.f, 2.f},
        {1.f, 0.f, 2.f},
        {1.f, 1.f, 2.f},
        {0.f, 1.f, 2.f},
    };
    std::vector<rerun::Vec3D> strip2 = {
        {0.f, 0.f, 0.f},
        {0.f, 0.f, 1.f},
        {1.f, 0.f, 0.f},
        {1.f, 0.f, 1.f},
        {1.f, 1.f, 0.f},
        {1.f, 1.f, 1.f},
        {0.f, 1.f, 0.f},
        {0.f, 1.f, 1.f},
    };
    rec.log(
        "strips",
        // TODO: Figure out how to avoid the extra vector
        rerun::LineStrips3D(std::vector{strip1, strip2})
            .with_colors({0xFF0000FF, 0x00FF00FF})
            .with_radii({0.025f, 0.005f})
            .with_labels({"one strip here", "and one strip there"})
    );
}
