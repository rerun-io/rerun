// Log a batch of capsules.

#include <rerun.hpp>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_capsule3d_batch");
    rec.spawn().exit_on_failure();

    rec.log(
        "capsules",
        rerun::Capsules3D::from_lengths_and_radii(
            {0.0f, 2.0f, 4.0f, 6.0f, 8.0f},
            {1.0f, 0.5f, 0.5f, 0.5f, 1.0f}
        )
            .with_colors({
                rerun::Rgba32(255, 0, 0),
                rerun::Rgba32(188, 188, 0),
                rerun::Rgba32(0, 255, 0),
                rerun::Rgba32(0, 188, 188),
                rerun::Rgba32(0, 0, 255),
            })
            .with_translations({
                {0.0f, 0.0f, 0.0f},
                {2.0f, 0.0f, 0.0f},
                {4.0f, 0.0f, 0.0f},
                {6.0f, 0.0f, 0.0f},
                {8.0f, 0.0f, 0.0f},
            })
            .with_rotation_axis_angles({
                rerun::RotationAxisAngle(),
                rerun::RotationAxisAngle({1.0f, 0.0f, 0.0f}, rerun::Angle::degrees(-22.5)),
                rerun::RotationAxisAngle({1.0f, 0.0f, 0.0f}, rerun::Angle::degrees(-45.0)),
                rerun::RotationAxisAngle({1.0f, 0.0f, 0.0f}, rerun::Angle::degrees(-67.5)),
                rerun::RotationAxisAngle({1.0f, 0.0f, 0.0f}, rerun::Angle::degrees(-90.0)),
            })
    );
}
