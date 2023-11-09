#include "transform3d.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace archetypes {

#ifdef 0
        static const Transform3D IDENTITY;

        /// New 3D transform from translation/matrix datatype.
        ///
        /// \param translation_and_mat3x3 Combined translation/matrix.
        Transform3D(const datatypes::TranslationAndMat3x3& translation_and_mat3x3)
            : Transform3D(datatypes::Transform3D::translation_and_mat3x3(translation_and_mat3x3)) {}

        /// Creates a new 3D transform from translation and matrix provided as 3 columns.
        ///
        /// \param translation \çopydoc datatypes::TranslationAndMat3x3::translation
        /// \param columns Column vectors of 3x3 matrix.
        /// \param from_parent \copydoc datatypes::TranslationRotationScale3D::from_parent
        ///
        /// _Implementation note:_ This overload is necessary, otherwise the array may be
        /// interpreted as bool and call the wrong overload.
        Transform3D(
            const datatypes::Vec3D& translation, const datatypes::Vec3D (&columns)[3],
            bool from_parent = false
        )
            : Transform3D(datatypes::TranslationAndMat3x3(translation, columns, from_parent)) {}

        /// Creates a new 3D transform from translation/matrix.
        ///
        /// \param translation \çopydoc datatypes::TranslationAndMat3x3::translation
        /// \param from_parent \copydoc datatypes::TranslationAndMat3x3::from_parent
        Transform3D(
            const datatypes::Vec3D& translation, const datatypes::Mat3x3& matrix,
            bool from_parent = false
        )
            : Transform3D(datatypes::TranslationAndMat3x3(translation, matrix, from_parent)) {}

        /// From translation only.
        ///
        /// \param translation \çopydoc datatypes::TranslationRotationScale3D::translation
        /// \param from_parent \copydoc datatypes::TranslationRotationScale3D::from_parent
        Transform3D(const datatypes::Vec3D& translation, bool from_parent = false)
            : Transform3D(datatypes::TranslationRotationScale3D(translation, from_parent)) {}

        /// From 3x3 matrix only.
        ///
        /// \param matrix \copydoc datatypes::TranslationAndMat3x3::matrix
        /// \param from_parent \copydoc datatypes::TranslationAndMat3x3::from_parent
        Transform3D(const datatypes::Mat3x3& matrix, bool from_parent = false)
            : Transform3D(datatypes::TranslationAndMat3x3(matrix, from_parent)) {}

        /// From 3x3 matrix provided as 3 columns only.
        ///
        /// \param columns Column vectors of 3x3 matrix.
        /// \param from_parent \copydoc datatypes::TranslationRotationScale3D::from_parent
        Transform3D(const datatypes::Vec3D (&columns)[3], bool from_parent = false)
            : Transform3D(datatypes::TranslationAndMat3x3(columns, from_parent)) {}

        /// New 3D transform from translation/rotation/scale datatype.
        /// \param translation_rotation_scale3d Combined translation/rotation/scale.
        Transform3D(const datatypes::TranslationRotationScale3D& translation_rotation_scale3d)
            : Transform3D(
                  datatypes::Transform3D::translation_rotation_scale(translation_rotation_scale3d)
              ) {}

        /// Creates a new 3D transform from translation/rotation/scale.
        ///
        /// \param translation \copydoc datatypes::TranslationRotationScale3D::translation
        /// \param rotation \copydoc datatypes::TranslationRotationScale3D::rotation
        /// \param scale \copydoc datatypes::TranslationRotationScale3D::scale
        /// \param from_parent \copydoc datatypes::TranslationRotationScale3D::from_parent
        Transform3D(
            const datatypes::Vec3D& translation, const datatypes::Rotation3D& rotation,
            const datatypes::Scale3D& scale, bool from_parent = false
        )
            : Transform3D(
                  datatypes::TranslationRotationScale3D(translation, rotation, scale, from_parent)
              ) {}

        /// Creates a new 3D transform from translation/rotation/uniform-scale.
        ///
        /// \param translation \copydoc datatypes::TranslationRotationScale3D::translation
        /// \param rotation \copydoc datatypes::TranslationRotationScale3D::rotation
        /// \param uniform_scale Uniform scale factor that is applied to all axis equally.
        /// \param from_parent \copydoc datatypes::TranslationRotationScale3D::from_parent
        ///
        /// _Implementation note:_ This explicit overload prevents interpretation of the float as
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
        /// \param translation \copydoc datatypes::TranslationRotationScale3D::translation
        /// \param rotation \copydoc datatypes::TranslationRotationScale3D::rotation
        /// \param from_parent \copydoc datatypes::TranslationRotationScale3D::from_parent
        Transform3D(
            const datatypes::Vec3D& translation, const datatypes::Rotation3D& rotation,
            bool from_parent = false
        )
            : Transform3D(datatypes::TranslationRotationScale3D(translation, rotation, from_parent)
              ) {}

        /// From translation & scale only.
        ///
        /// \param translation \copydoc datatypes::TranslationRotationScale3D::translation
        /// \param scale datatypes::TranslationRotationScale3D::scale
        /// \param from_parent \copydoc datatypes::TranslationRotationScale3D::from_parent
        Transform3D(
            const datatypes::Vec3D& translation, const datatypes::Scale3D& scale,
            bool from_parent = false
        )
            : Transform3D(datatypes::TranslationRotationScale3D(translation, scale, from_parent)) {}

        /// From translation & uniform scale only.
        ///
        /// \param translation \copydoc datatypes::TranslationRotationScale3D::translation
        /// \param uniform_scale Uniform scale factor that is applied to all axis equally.
        /// \param from_parent \copydoc datatypes::TranslationRotationScale3D::from_parent
        ///
        /// _Implementation note:_ This explicit overload prevents interpretation of the float as
        /// bool, leading to a call to the wrong overload.
        Transform3D(
            const datatypes::Vec3D& translation, float uniform_scale, bool from_parent = false
        )
            : Transform3D(
                  datatypes::TranslationRotationScale3D(translation, uniform_scale, from_parent)
              ) {}

        /// From rotation & scale.
        ///
        /// \param from_parent \copydoc datatypes::TranslationRotationScale3D::from_parent
        Transform3D(
            const datatypes::Rotation3D& rotation, const datatypes::Scale3D& scale,
            bool from_parent = false
        )
            : Transform3D(datatypes::TranslationRotationScale3D(rotation, scale, from_parent)) {}

        /// From rotation & uniform scale.
        ///
        /// \param rotation \copydoc datatypes::TranslationRotationScale3D::rotation
        /// \param uniform_scale Uniform scale factor that is applied to all axis equally.
        /// \param from_parent \copydoc datatypes::TranslationRotationScale3D::from_parent
        ///
        /// _Implementation note:_ This explicit overload prevents interpretation of the float as
        /// bool, leading to a call to the wrong overload.
        Transform3D(
            const datatypes::Rotation3D& rotation, float uniform_scale, bool from_parent = false
        )
            : Transform3D(
                  datatypes::TranslationRotationScale3D(rotation, uniform_scale, from_parent)
              ) {}

        /// From rotation only.
        ///
        /// \param rotation \copydoc datatypes::TranslationRotationScale3D::rotation
        /// \param from_parent \copydoc datatypes::TranslationRotationScale3D::from_parent
        Transform3D(const datatypes::Rotation3D& rotation, bool from_parent = false)
            : Transform3D(datatypes::TranslationRotationScale3D(rotation, from_parent)) {}

        /// From scale only.
        ///
        /// \param scale \copydoc datatypes::TranslationRotationScale3D::from_parent
        /// \param from_parent \copydoc datatypes::TranslationRotationScale3D::scale
        Transform3D(const datatypes::Scale3D& scale, bool from_parent = false)
            : Transform3D(datatypes::TranslationRotationScale3D(scale, from_parent)) {}

        // </CODEGEN_COPY_TO_HEADER>
#endif

        const Transform3D Transform3D::IDENTITY =
            Transform3D(datatypes::TranslationAndMat3x3::IDENTITY);

    } // namespace archetypes
} // namespace rerun
