// Logs a `Transform3D` archetype for roundtrip checks.

#include <rerun/archetypes/transform3d.hpp>
#include <rerun/recording_stream.hpp>

#include <cmath>

namespace rr = rerun;

int main(int argc, char** argv) {
    auto rec_stream = rr::RecordingStream("rerun_example_roundtrip_transform3d");
    rec_stream.save(argv[1]).throw_on_failure();

    rec_stream.log(
        "translation_and_mat3x3/identity",
        rr::archetypes::Transform3D(rr::datatypes::TranslationAndMat3x3::IDENTITY)
    );

    rec_stream.log(
        "translation_and_mat3x3/translation",
        rr::archetypes::Transform3D({1.0f, 2.0f, 3.0f}, true)
    );

    rec_stream.log(
        "translation_and_mat3x3/rotation",
        rr::archetypes::Transform3D({
            {1.0f, 4.0f, 7.0f},
            {2.0f, 5.0f, 8.0f},
            {3.0f, 6.0f, 9.0f},
        })
    );

    rec_stream.log(
        "translation_rotation_scale/identity",
        rr::archetypes::Transform3D(rr::datatypes::TranslationRotationScale3D::IDENTITY)
    );

    rec_stream.log(
        "translation_rotation_scale/translation_scale",
        rr::archetypes::Transform3D(
            {1.0f, 2.0f, 3.0f},
            rr::datatypes::Scale3D::uniform(42.0f),
            true
        )
    );

    rec_stream.log(
        "translation_rotation_scale/rigid",
        rr::archetypes::Transform3D(
            {1.0f, 2.0f, 3.0f},
            rr::datatypes::RotationAxisAngle(
                {0.2f, 0.2f, 0.8f},
                rr::datatypes::Angle::radians(static_cast<float>(M_PI))
            )
        )
    );

    rec_stream.log(
        "translation_rotation_scale/affine",
        rr::archetypes::Transform3D(
            {1.0f, 2.0f, 3.0f},
            rr::datatypes::RotationAxisAngle(
                {0.2f, 0.2f, 0.8f},
                rr::datatypes::Angle::radians(static_cast<float>(M_PI))
            ),
            42.0f,
            true
        )
    );
}
