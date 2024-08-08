// Log different transforms with visualized coordinates axes.

#include <rerun.hpp>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_transform3d_axes");
    rec.spawn().exit_on_failure();

    auto base_axes = rerun::Transform3D().with_axis_length(1.0);
    auto other_axes = rerun::Transform3D().with_axis_length(0.5);

    rec.set_time_sequence("step", 0);

    rec.log("base", base_axes);

    for (int deg = 0; deg < 360; deg++) {
        rec.set_time_sequence("step", deg);

        rec.log(
            "base/rotated",
            other_axes.with_rotation(rerun::RotationAxisAngle(
                {1.0f, 1.0f, 1.0f},
                rerun::Angle::degrees(static_cast<float>(deg))
            ))
        );

        rec.log("base/rotated/translated", other_axes.with_translation({2.0f, 0.0f, 0.0f}));
    }
}
