//! Update a transform over time, in a single operation.
//!
//! This is semantically equivalent to the `transform3d_row_updates` example, albeit much faster.

#include <cmath>
#include <numeric>
#include <vector>

#include <rerun.hpp>

float truncated_radians(int deg) {
    auto degf = static_cast<float>(deg);
    const auto pi = 3.14159265358979323846f;
    return static_cast<float>(static_cast<int>(degf * pi / 180.0f * 1000.0f)) / 1000.0f;
}

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_transform3d_column_updates");
    rec.spawn().exit_on_failure();

    rec.set_time_sequence("tick", 0);
    rec.log(
        "box",
        rerun::Boxes3D::from_half_sizes({{4.f, 2.f, 1.0f}}).with_fill_mode(rerun::FillMode::Solid),
        rerun::TransformAxes3D(10.0)
    );

    std::vector<std::array<float, 3>> translations;
    std::vector<rerun::RotationAxisAngle> rotations;
    for (int t = 0; t < 100; t++) {
        translations.push_back({0.0f, 0.0f, static_cast<float>(t) / 10.0f});
        rotations.push_back(rerun::RotationAxisAngle(
            {0.0f, 1.0f, 0.0f},
            rerun::Angle::radians(truncated_radians(t * 4))
        ));
    }

    std::vector<int64_t> ticks(100);
    std::iota(ticks.begin(), ticks.end(), 1);

    rec.send_columns(
        "box",
        rerun::TimeColumn::from_sequence("tick", ticks),
        rerun::Transform3D()
            .with_many_translation(translations)
            .with_many_rotation_axis_angle(rotations)
            .columns()
    );
}
