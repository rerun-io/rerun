// Logs a `Transform3D` archetype for roundtrip checks.

#include <rerun/archetypes/transform3d.hpp>
#include <rerun/recording_stream.hpp>

#include <cmath>

int main(int, char** argv) {
    auto rec = rerun::RecordingStream("rerun_example_roundtrip_transform3d");
    rec.save(argv[1]).throw_on_failure();

    rec.log(
        "translation_and_mat3x3/identity",
        rerun::archetypes::Transform3D(rerun::datatypes::TranslationAndMat3x3::IDENTITY)
    );

    rec.log(
        "translation_and_mat3x3/translation",
        rerun::archetypes::Transform3D(
            rerun::datatypes::TranslationAndMat3x3({1.0f, 2.0f, 3.0f}, true)
        )
    );

    rec.log(
        "translation_and_mat3x3/rotation",
        rerun::archetypes::Transform3D({
            {1.0f, 4.0f, 7.0f},
            {2.0f, 5.0f, 8.0f},
            {3.0f, 6.0f, 9.0f},
        })
    );

    rec.log(
        "translation_rotation_scale/identity",
        rerun::archetypes::Transform3D(rerun::datatypes::TranslationRotationScale3D::IDENTITY)
    );

    rec.log(
        "translation_rotation_scale/translation_scale",
        rerun::archetypes::Transform3D(
            {1.0f, 2.0f, 3.0f},
            rerun::datatypes::Scale3D::uniform(42.0f),
            true
        )
    );

    rec.log(
        "translation_rotation_scale/rigid",
        rerun::archetypes::Transform3D(
            {1.0f, 2.0f, 3.0f},
            rerun::datatypes::RotationAxisAngle(
                {0.2f, 0.2f, 0.8f},
                rerun::datatypes::Angle::radians(static_cast<float>(M_PI))
            )
        )
    );

    rec.log(
        "translation_rotation_scale/affine",
        rerun::archetypes::Transform3D(
            {1.0f, 2.0f, 3.0f},
            rerun::datatypes::RotationAxisAngle(
                {0.2f, 0.2f, 0.8f},
                rerun::datatypes::Angle::radians(static_cast<float>(M_PI))
            ),
            42.0f,
            true
        )
    );
}
