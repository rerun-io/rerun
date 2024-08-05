// Log a simple 3D box with a regular & leaf transform.

#include <rerun.hpp>
#include <rerun/demo_utils.hpp>

int main() {
    const auto rec = rerun::RecordingStream("rerun_example_leaf_transform3d_combined");
    rec.set_time_sequence("frame", 0);

    // Log a box and points further down in the hierarchy.
    rec.log("world/box", rerun::Boxes3D::from_half_sizes({{1.0, 1.0, 1.0}}));
    rec.log(
        "world/box/points",
        rerun::Points3D(rerun::demo::grid3d<rerun::Position3D, float>(-10.0f, 10.0f, 10))
    );

    for (int i = 0; i < 180; ++i) {
        rec.set_time_sequence("frame", i);

        // Log a regular transform which affects both the box and the points.
        rec.log(
            "world/box",
            rerun::Transform3D::from_rotation(rerun::RotationAxisAngle{
                {0.0f, 0.0f, 1.0f},
                rerun::Angle::degrees(static_cast<float>(i) * 2.0f)})
        );

        // Log an leaf transform which affects only the box.
        rec.log(
            "world/box",
            rerun::LeafTransforms3D().with_translations(
                {{0.0f, 0.0f, std::abs(static_cast<float>(i) * 0.1f - 5.0f) - 5.0f}}
            )
        );
    }
}
