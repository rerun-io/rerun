// Log a batch of oriented bounding boxes.

#include <rerun.hpp>

int main() {
    auto rec = rerun::RecordingStream("rerun_example_box3d_batch");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    rec.log(
        "batch",
        rerun::Boxes3D::from_centers_and_half_sizes(
            {{2.0f, 0.0f, 0.0f}, {-2.0f, 0.0f, 0.0f}, {0.0f, 0.0f, 2.0f}},
            {{2.0f, 2.0f, 1.0f}, {1.0f, 1.0f, 0.5f}, {2.0f, 0.5f, 1.0f}}
        )
            .with_rotations({
                rerun::datatypes::Quaternion::IDENTITY,
                // 45 degrees around Z
                rerun::datatypes::Quaternion(0.0f, 0.0f, 0.382683f, 0.923880f),
                rerun::datatypes::RotationAxisAngle(
                    {0.0f, 1.0f, 0.0f},
                    rerun::datatypes::Angle::degrees(30.0f)
                ),
            })
            .with_radii({0.025f})
            .with_colors({
                rerun::datatypes::Rgba32(255, 0, 0),
                rerun::datatypes::Rgba32(0, 255, 0),
                rerun::datatypes::Rgba32(0, 0, 255),
            })
            .with_labels({"red", "green", "blue"})
    );
}
