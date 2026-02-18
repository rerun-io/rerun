//! Update specific properties of a transform over time.

#include <rerun.hpp>

float truncated_radians(int deg) {
    auto degf = static_cast<float>(deg);
    const auto pi = 3.14159265358979323846f;
    return static_cast<float>(static_cast<int>(degf * pi / 180.0f * 1000.0f)) / 1000.0f;
}

int main(int argc, char* argv[]) {
    const auto rec = rerun::RecordingStream("rerun_example_transform3d_partial_updates");
    rec.spawn().exit_on_failure();

    // Set up a 3D box.
    rec.log(
        "box",
        rerun::Boxes3D::from_half_sizes({{4.f, 2.f, 1.0f}}).with_fill_mode(rerun::FillMode::Solid)
    );

    // Update only the rotation of the box.
    for (int deg = 0; deg <= 45; deg++) {
        auto rad = truncated_radians(deg * 4);
        rec.log(
            "box",
            rerun::Transform3D::from_rotation(
                rerun::RotationAxisAngle({0.0f, 1.0f, 0.0f}, rerun::Angle::radians(rad))
            )
        );
    }

    // Update only the position of the box.
    for (int t = 0; t <= 50; t++) {
        rec.log(
            "box",
            rerun::Transform3D::from_translation({0.0f, 0.0f, static_cast<float>(t) / 10.0f})
        );
    }

    // Update only the rotation of the box.
    for (int deg = 0; deg <= 45; deg++) {
        auto rad = truncated_radians((deg + 45) * 4);
        rec.log(
            "box",
            rerun::Transform3D::from_rotation(
                rerun::RotationAxisAngle({0.0f, 1.0f, 0.0f}, rerun::Angle::radians(rad))
            )
        );
    }

    // Clear all of the box's attributes.
    rec.log("box", rerun::Transform3D::clear_fields());
}
