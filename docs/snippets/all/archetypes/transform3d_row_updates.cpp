//! Update a transform over time.
//!
//! See also the `transform3d_column_updates` example, which achieves the same thing in a single operation.

#include <rerun.hpp>

float truncated_radians(int deg) {
    auto degf = static_cast<float>(deg);
    const auto pi = 3.14159265358979323846f;
    return static_cast<float>(static_cast<int>(degf * pi / 180.0f * 1000.0f)) / 1000.0f;
}

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_transform3d_row_updates");
    rec.spawn().exit_on_failure();

    rec.set_time_sequence("tick", 0);
    rec.log(
        "box",
        rerun::Boxes3D::from_half_sizes({{4.f, 2.f, 1.0f}}).with_fill_mode(rerun::FillMode::Solid),
        rerun::TransformAxes3D(10.0)
    );

    for (int t = 0; t < 100; t++) {
        rec.set_time_sequence("tick", t + 1);
        rec.log(
            "box",
            rerun::Transform3D()
                .with_translation({0.0f, 0.0f, static_cast<float>(t) / 10.0f})
                .with_rotation_axis_angle(rerun::RotationAxisAngle(
                    {0.0f, 1.0f, 0.0f},
                    rerun::Angle::radians(truncated_radians(t * 4))
                ))
        );
    }
}
