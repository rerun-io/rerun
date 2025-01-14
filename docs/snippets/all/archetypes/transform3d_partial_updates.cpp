// Log different transforms with visualized coordinates axes.

#include <cmath>
#include <rerun.hpp>

float truncated_radians(float deg) {
    return static_cast<float>(static_cast<int>(deg * M_PI / 180.0f * 1000.0f)) / 1000.0f;
}

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_transform3d_axes");
    rec.spawn().exit_on_failure();

    auto step = 0;

    rec.set_time_sequence("step", step);
    rec.log(
        "box",
        rerun::Boxes3D::from_half_sizes({{4.f, 2.f, 1.0f}}).with_fill_mode(rerun::FillMode::Solid),
        rerun::Transform3D().with_axis_length(10.0)
    );

    for (int deg = 0; deg <= 45; deg++) {
        step++;
        rec.set_time_sequence("step", step);

        auto rad = truncated_radians(deg * 4);
        // TODO: update_fields
        rec.log(
            "box",
            rerun::Transform3D().with_rotation_axis_angle(
                rerun::RotationAxisAngle({0.0f, 1.0f, 0.0f}, rerun::Angle::radians(rad))
            )
        );
    }

    for (int t = 0; t <= 45; t++) {
        step++;
        rec.set_time_sequence("step", step);
        rec.log(
            "box",
            rerun::Transform3D().with_translation({0.0f, 0.0f, static_cast<float>(t) / 10.0f})
        );
    }

    for (int deg = 0; deg <= 45; deg++) {
        step++;
        rec.set_time_sequence("step", step);

        auto rad = truncated_radians((deg + 45) * 4);
        // TODO: update_fields
        rec.log(
            "box",
            rerun::Transform3D().with_rotation_axis_angle(
                rerun::RotationAxisAngle({0.0f, 1.0f, 0.0f}, rerun::Angle::radians(rad))
            )
        );
    }

    step++;
    rec.set_time_sequence("step", step);
    // TODO: clear_fields
    rec.log("box", rerun::Transform3D().with_axis_length(15.0));
}
