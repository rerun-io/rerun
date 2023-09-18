// Log a batch of oriented bounding boxes.

#include <rerun.hpp>

namespace rr = rerun;

int main() {
    auto rec = rr::RecordingStream("rerun_example_box3d_batch");
    rec.connect("127.0.0.1:9876").throw_on_failure();

    rec.log(
        "batch",
        rr::Boxes3D::from_centers_and_half_sizes(
            {{2.0f, 0.0f, 0.0f}, {-2.0f, 0.0f, 0.0f}, {0.0f, 0.0f, 2.0f}},
            {{2.0f, 2.0f, 1.0f}, {1.0f, 1.0f, 0.5f}, {2.0f, 0.5f, 1.0f}}
        )
            .with_rotations({
                rr::datatypes::Rotation3D::IDENTITY,
                rr::datatypes::Quaternion(0.0f, 0.0f, 0.382683f, 0.923880f), // 45 degrees around Z
                rr::datatypes::RotationAxisAngle(
                    {0.0f, 1.0f, 1.0f},
                    rr::datatypes::Angle::degrees(30.0f)
                ),
            })
            .with_radii(0.025f)
            .with_colors({
                rr::datatypes::Color(255, 0, 0),
                rr::datatypes::Color(0, 255, 0),
                rr::datatypes::Color(0, 0, 255),
            })
            .with_labels({"red", "green", "blue"})
    );
}
