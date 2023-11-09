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

            // <CODEGEN_COPY_TO_HEADER>

            static const Transform3D IDENTITY;

            /// New 3D transform from translation/matrix datatype.
            Transform3D(const datatypes::TranslationAndMat3x3& translation_and_mat3x3)
                : Transform3D(datatypes::Transform3D::translation_and_mat3x3(translation_and_mat3x3)
                  ) {}

            /// Creates a new 3D transform from translation and matrix provided as 3 columns.
            ///
            /// \param from_parent If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            /// parent.
            ///
            /// Implementation note: This overload is necessary, otherwise the array may be
            /// interpreted as bool and call the wrong overload.
            Transform3D(
                const datatypes::Vec3D& translation, const datatypes::Vec3D (&columns)[3],
                bool from_parent = false
            )
                : Transform3D(datatypes::TranslationAndMat3x3(translation, columns, from_parent)) {}

            /// Creates a new 3D transform from translation/matrix.
            ///
            /// \param from_parent If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            /// parent.
            Transform3D(
                const datatypes::Vec3D& translation, const datatypes::Mat3x3& matrix,
                bool from_parent = false
            )
                : Transform3D(datatypes::TranslationAndMat3x3(translation, matrix, from_parent)) {}

            /// From translation only.
            ///
            /// \param from_parent If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            /// parent.
            Transform3D(const datatypes::Vec3D& translation, bool from_parent = false)
                : Transform3D(datatypes::TranslationRotationScale3D(translation, from_parent)) {}

            /// From 3x3 matrix only.
            ///
            /// \param from_parent If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            /// parent.
            Transform3D(const datatypes::Mat3x3& matrix, bool from_parent = false)
                : Transform3D(datatypes::TranslationAndMat3x3(matrix, from_parent)) {}

            /// From 3x3 matrix provided as 3 columns only.
            ///
            /// \param from_parent If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            /// parent.
            Transform3D(const datatypes::Vec3D (&columns)[3], bool from_parent = false)
                : Transform3D(datatypes::TranslationAndMat3x3(columns, from_parent)) {}

            /// New 3D transform from translation/rotation/scale datatype.
            Transform3D(const datatypes::TranslationRotationScale3D& translation_rotation_scale3d)
                : Transform3D(datatypes::Transform3D::translation_rotation_scale(
                      translation_rotation_scale3d
                  )) {}

            /// Creates a new 3D transform from translation/rotation/scale.
            ///
            /// \param from_parent If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            /// parent.
            Transform3D(
                const datatypes::Vec3D& translation, const datatypes::Rotation3D& rotation,
                const datatypes::Scale3D& scale, bool from_parent = false
            )
                : Transform3D(datatypes::TranslationRotationScale3D(
                      translation, rotation, scale, from_parent
                  )) {}

            /// Creates a new 3D transform from translation/rotation/uniform-scale.
            ///
            /// \param from_parent If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            /// parent.
            ///
            /// Implementation note: This explicit overload prevents interpretation of the float as
            /// bool, leading to a call to the wrong overload.
            Transform3D(
                const datatypes::Vec3D& translation, const datatypes::Rotation3D& rotation,
                float uniform_scale, bool from_parent = false
            )
                : Transform3D(datatypes::TranslationRotationScale3D(
                      translation, rotation, uniform_scale, from_parent
                  )) {}

            /// Creates a new rigid transform (translation & rotation only).
            ///
            /// \param from_parent If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            /// parent.
            Transform3D(
                const datatypes::Vec3D& translation, const datatypes::Rotation3D& rotation,
                bool from_parent = false
            )
                : Transform3D(
                      datatypes::TranslationRotationScale3D(translation, rotation, from_parent)
                  ) {}

            /// From translation & scale only.
            ///
            /// \param from_parent If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            /// parent.
            Transform3D(
                const datatypes::Vec3D& translation, const datatypes::Scale3D& scale,
                bool from_parent = false
            )
                : Transform3D(datatypes::TranslationRotationScale3D(translation, scale, from_parent)
                  ) {}

            /// From translation & uniform scale only.
            ///
            /// \param from_parent If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            /// parent.
            ///
            /// Implementation note: This explicit overload prevents interpretation of the float as
            /// bool, leading to a call to the wrong overload.
            Transform3D(
                const datatypes::Vec3D& translation, float uniform_scale, bool from_parent = false
            )
                : Transform3D(
                      datatypes::TranslationRotationScale3D(translation, uniform_scale, from_parent)
                  ) {}

            /// From rotation & scale.
            ///
            /// \param from_parent If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            /// parent.
            Transform3D(
                const datatypes::Rotation3D& rotation, const datatypes::Scale3D& scale,
                bool from_parent = false
            )
                : Transform3D(datatypes::TranslationRotationScale3D(rotation, scale, from_parent)) {
            }

            /// From rotation & uniform scale.
            ///
            /// \param from_parent If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            /// parent.
            ///
            /// Implementation note: This explicit overload prevents interpretation of the float as
            /// bool, leading to a call to the wrong overload.
            Transform3D(
                const datatypes::Rotation3D& rotation, float uniform_scale, bool from_parent = false
            )
                : Transform3D(
                      datatypes::TranslationRotationScale3D(rotation, uniform_scale, from_parent)
                  ) {}

            /// From rotation only.
            ///
            /// \param from_parent If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            /// parent.
            Transform3D(const datatypes::Rotation3D& rotation, bool from_parent = false)
                : Transform3D(datatypes::TranslationRotationScale3D(rotation, from_parent)) {}

            /// From scale only.
            ///
            /// \param from_parent If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            /// parent.
            Transform3D(const datatypes::Scale3D& scale, bool from_parent = false)
                : Transform3D(datatypes::TranslationRotationScale3D(scale, from_parent)) {}

            // </CODEGEN_COPY_TO_HEADER>
        };

#undef Transform3DExt
#else
#define Transform3DExt Transform3D
#endif

        const Transform3DExt Transform3DExt::IDENTITY =
            Transform3DExt(datatypes::TranslationAndMat3x3::IDENTITY);

    } // namespace archetypes
} // namespace rerun
