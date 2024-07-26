// Logs a `Transform3D` archetype for roundtrip checks.

#include <rerun/archetypes/transform3d.hpp>
#include <rerun/recording_stream.hpp>

constexpr float PI = 3.14159265358979323846264338327950288f;

int main(int, char** argv) {
    const auto rec = rerun::RecordingStream("rerun_example_roundtrip_transform3d");
    rec.save(argv[1]).exit_on_failure();

    rec.log(
        "transform/translation",
        rerun::archetypes::Transform3D::from_translation({1.0f, 2.0f, 3.0f})
            .with_relation(rerun::components::TransformRelation::ChildFromParent)
    );

    rec.log(
        "transform/rotation",
        rerun::archetypes::Transform3D::from_mat3x3({
            {1.0f, 4.0f, 7.0f},
            {2.0f, 5.0f, 8.0f},
            {3.0f, 6.0f, 9.0f},
        })
    );

    rec.log(
        "transform/translation_scale",
        rerun::archetypes::Transform3D::from_translation_scale(
            {1.0f, 2.0f, 3.0f},
            rerun::components::Scale3D::uniform(42.0f)
        )
            .with_relation(rerun::components::TransformRelation::ChildFromParent)
    );

    rec.log(
        "transform/rigid",
        rerun::archetypes::Transform3D::from_translation_rotation(
            {1.0f, 2.0f, 3.0f},
            rerun::datatypes::RotationAxisAngle(
                {0.2f, 0.2f, 0.8f},
                rerun::datatypes::Angle::radians(PI)
            )
        )
    );

    rec.log(
        "transform/affine",
        rerun::archetypes::Transform3D::from_translation_rotation_scale(
            {1.0f, 2.0f, 3.0f},
            rerun::datatypes::RotationAxisAngle(
                {0.2f, 0.2f, 0.8f},
                rerun::datatypes::Angle::radians(PI)
            ),
            rerun::components::Scale3D::uniform(42.0)
        )
            .with_relation(rerun::components::TransformRelation::ChildFromParent)
    );
}
