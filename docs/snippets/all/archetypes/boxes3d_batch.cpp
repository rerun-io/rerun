// Log a batch of oriented bounding boxes.

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_box3d_batch");
    rec.spawn().exit_on_failure();

    rec.log(
        "batch",
        rerun::Boxes3D::from_centers_and_half_sizes(
            {{2.0f, 0.0f, 0.0f}, {-2.0f, 0.0f, 0.0f}, {0.0f, 0.0f, 2.0f}},
            {{2.0f, 2.0f, 1.0f}, {1.0f, 1.0f, 0.5f}, {2.0f, 0.5f, 1.0f}}
        )
            .with_quaternions({
                rerun::Quaternion::IDENTITY,
                // 45 degrees around Z
                rerun::Quaternion::from_xyzw(0.0f, 0.0f, 0.382683f, 0.923880f),
            })
            .with_radii({0.025f})
            .with_colors({
                rerun::Rgba32(255, 0, 0),
                rerun::Rgba32(0, 255, 0),
                rerun::Rgba32(0, 0, 255),
            })
            .with_fill_mode(rerun::FillMode::Solid)
            .with_labels({"red", "green", "blue"})
    );
}
