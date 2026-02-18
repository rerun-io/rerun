// Log different transforms with visualized coordinates axes.

#include <rerun.hpp>

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_transform3d_axes");
    rec.spawn().exit_on_failure();

    rec.set_time_sequence("step", 0);

    rec.log("base", rerun::Transform3D(), rerun::TransformAxes3D(1.0));

    for (int deg = 0; deg < 360; deg++) {
        rec.set_time_sequence("step", deg);

        rec.log(
            "base/rotated",
            rerun::Transform3D().with_rotation_axis_angle(rerun::RotationAxisAngle(
                {1.0f, 1.0f, 1.0f},
                rerun::Angle::degrees(static_cast<float>(deg))
            )),
            rerun::TransformAxes3D(0.5)
        );

        rec.log(
            "base/rotated/translated",
            rerun::Transform3D().with_translation({2.0f, 0.0f, 0.0f}),
            rerun::TransformAxes3D(0.5)
        );
    }
}
