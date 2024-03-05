
#include <rerun/archetypes/boxes3d.hpp>
#include <rerun/recording_stream.hpp>

int main(int, char** argv) {
    const auto rec = rerun::RecordingStream("rerun_example_roundtrip_box3d");
    rec.save(argv[1]).exit_on_failure();

    rec.log(
        "boxes3d",
        rerun::archetypes::Boxes3D::from_half_sizes({{10.f, 9.f, 8.f}, {5.f, -5.f, 5.f}})
            .with_centers({{0.f, 0.f, 0.f}, {-1.f, 1.f, -2.f}})
            .with_rotations({
                rerun::datatypes::Quaternion::from_xyzw(0.f, 1.f, 2.f, 3.f),
                rerun::datatypes::RotationAxisAngle(
                    {0.f, 1.f, 2.f},
                    rerun::datatypes::Angle::degrees(45.f)
                ),
            })
            .with_colors({0xAA0000CC, 0x00BB00DD})
            .with_labels({"hello", "friend"})
            .with_radii({0.1f, 0.01f})
            .with_class_ids({126, 127})
    );
}
