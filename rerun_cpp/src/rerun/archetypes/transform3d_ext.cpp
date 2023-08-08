#include "transform3d.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef EDIT_EXTENSION
        struct Transform3DExt {
            rerun::components::Transform3D transform;

            Transform3DExt(rerun::components::Transform3D _transform)
                : transform(std::move(_transform)) {}

#define Transform3D Transform3DExt

            // [CODEGEN COPY TO HEADER START]

            static const Transform3D IDENTITY;

            /// New transform from translation and arbitrary 3x3 matrix.
            Transform3D(const rerun::datatypes::TranslationAndMat3x3& translation_and_mat3x3)
                : Transform3D(
                      rerun::datatypes::Transform3D::translation_and_mat3x3(translation_and_mat3x3)
                  ) {}

            /// New transform from translation/rotation/scale.
            Transform3D(
                const rerun::datatypes::TranslationRotationScale3D& translation_rotation_scale3d
            )
                : Transform3D(rerun::datatypes::Transform3D::translation_rotation_scale(
                      translation_rotation_scale3d
                  )) {}

            // [CODEGEN COPY TO HEADER END]
        };

#undef Transform3DExt
#else
#define Transform3DExt Transform3D
#endif

        const Transform3DExt Transform3DExt::IDENTITY =
            Transform3DExt(datatypes::TranslationRotationScale3D::IDENTITY);

    } // namespace archetypes
} // namespace rerun
