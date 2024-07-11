#include "transform3d.hpp"

// <CODEGEN_COPY_TO_HEADER>
#include "../rerun_sdk_export.hpp"

// </CODEGEN_COPY_TO_HEADER>

namespace rerun {
    namespace archetypes {

#if 0
        // <CODEGEN_COPY_TO_HEADER>

        /// Identity transformation.
        ///
        /// Applying this transform does not alter an entity's transformation.
        RERUN_SDK_EXPORT static const Transform3D IDENTITY;

        /// Creates a new 3D transform from translation and matrix provided as 3 columns.
        ///
        /// \param _translation \çopydoc Transform3D::translation
        /// \param columns Column vectors of 3x3 matrix.
        /// \param from_parent \copydoc datatypes::TranslationRotationScale3D::from_parent
        ///
        /// _Implementation note:_ This overload is necessary, otherwise the array may be
        /// interpreted as bool and call the wrong overload.
        Transform3D(
            const components::Translation3D& _translation, const datatypes::Vec3D (&columns)[3],
            bool from_parent = false
        )
            : transform(datatypes::TranslationRotationScale3D(from_parent)),
              mat3x3(Collection<components::TransformMat3x3>::take_ownership(
                  components::TransformMat3x3(columns)
              )),
              translation(Collection<components::Translation3D>::take_ownership(_translation)) {}

        /// Creates a new 3D transform from translation/matrix.
        ///
        /// \param _translation \çopydoc Transform3D::translation
        /// \param _mat3x3 \copydoc Transform3D::mat3x3
        /// \param from_parent \copydoc datatypes::TranslationRotationScale3D::from_parent
        Transform3D(
            const components::Translation3D& _translation,
            const components::TransformMat3x3& _mat3x3, bool from_parent = false
        )
            : transform(datatypes::TranslationRotationScale3D(from_parent)),
              mat3x3(Collection<components::TransformMat3x3>::take_ownership(_mat3x3)),
              translation(Collection<components::Translation3D>::take_ownership(_translation)) {}

        /// From a translation applied after a 3x3 matrix.
        ///
        /// \param translation \çopydoc Transform3D::translation
        /// \param mat3x3 \copydoc Transform3D::mat3x3
        static Transform3D from_translation_mat3x3(
            const components::Translation3D& translation, const components::TransformMat3x3& mat3x3
        ) {
            return Transform3D(translation, mat3x3, false);
        }

        /// From a translation applied after a 3x3 matrix provided as 3 columns.
        ///
        /// \param translation \çopydoc Transform3D::translation
        /// \param columns Column vectors of 3x3 matrix.
        static Transform3D from_translation_mat3x3(
            const components::Translation3D& translation, const datatypes::Vec3D (&columns)[3]
        ) {
            return Transform3D::from_translation_mat3x3(
                translation,
                components::TransformMat3x3(columns)
            );
        }

        /// From translation only.
        ///
        /// \param _translation \çopydoc Transform3D::translation
        /// \param from_parent \copydoc datatypes::TranslationRotationScale3D::from_parent
        Transform3D(const components::Translation3D& _translation, bool from_parent = false)
            : transform(datatypes::TranslationRotationScale3D(from_parent)),
              translation(Collection<components::Translation3D>::take_ownership(_translation)) {}

        /// From a translation.
        ///
        /// \param translation \çopydoc Transform3D::translation
        static Transform3D from_translation(const components::Translation3D& translation) {
            return Transform3D(translation, false);
        }

        /// From 3x3 matrix only.
        ///
        /// \param _mat3x3 \copydoc Transform3D::mat3x3
        /// \param from_parent \copydoc datatypes::TranslationRotationScale3D::from_parent
        Transform3D(const components::TransformMat3x3& _mat3x3, bool from_parent = false)
            : transform(datatypes::TranslationRotationScale3D(from_parent)),
              mat3x3(Collection<components::TransformMat3x3>::take_ownership(_mat3x3)) {}

        /// From 3x3 matrix only.
        ///
        /// \param mat3x3 \copydoc Transform3D::mat3x3
        static Transform3D from_mat3x3(const components::TransformMat3x3& mat3x3) {
            return Transform3D(mat3x3, false);
        }

        /// From 3x3 matrix provided as 3 columns only.
        ///
        /// \param columns Column vectors of 3x3 matrix.
        /// \param from_parent \copydoc datatypes::TranslationRotationScale3D::from_parent
        Transform3D(const datatypes::Vec3D (&columns)[3], bool from_parent = false)
            : transform(datatypes::TranslationRotationScale3D(from_parent)),
              mat3x3(Collection<components::TransformMat3x3>::take_ownership(
                  components::TransformMat3x3(columns)
              )) {}

        /// From 3x3 matrix provided as 3 columns only.
        ///
        /// \param columns Column vectors of 3x3 matrix.
        static Transform3D from_mat3x3(const datatypes::Vec3D (&columns)[3]) {
            return Transform3D::from_mat3x3(components::TransformMat3x3(columns));
        }

        /// Creates a new 3D transform from translation/rotation/scale.
        ///
        /// \param _translation \copydoc Transform3D::translation
        /// \param rotation \copydoc datatypes::TranslationRotationScale3D::rotation
        /// \param scale \copydoc datatypes::TranslationRotationScale3D::scale
        /// \param from_parent \copydoc datatypes::TranslationRotationScale3D::from_parent
        Transform3D(
            const components::Translation3D& _translation, const datatypes::Rotation3D& rotation,
            const datatypes::Scale3D& scale, bool from_parent = false
        )
            : transform(datatypes::TranslationRotationScale3D(rotation, scale, from_parent)),
              translation(Collection<components::Translation3D>::take_ownership(_translation)) {}

        /// Creates a new 3D transform from translation/rotation/uniform-scale.
        ///
        /// \param _translation \copydoc Transform3D::translation
        /// \param rotation \copydoc datatypes::TranslationRotationScale3D::rotation
        /// \param uniform_scale Uniform scale factor that is applied to all axis equally.
        /// \param from_parent \copydoc datatypes::TranslationRotationScale3D::from_parent
        ///
        /// _Implementation note:_ This explicit overload prevents interpretation of the float as
        /// bool, leading to a call to the wrong overload.
        Transform3D(
            const components::Translation3D& _translation, const datatypes::Rotation3D& rotation,
            float uniform_scale, bool from_parent = false
        )
            : transform(datatypes::TranslationRotationScale3D(rotation, uniform_scale, from_parent)
              ),
              translation(Collection<components::Translation3D>::take_ownership(_translation)) {}

        /// From a translation, applied after a rotation & scale, known as an affine transformation.
        ///
        /// \param translation \copydoc Transform3D::translation
        /// \param rotation \copydoc datatypes::TranslationRotationScale3D::rotation
        /// \param scale \copydoc datatypes::TranslationRotationScale3D::scale
        static Transform3D from_translation_rotation_scale(
            const components::Translation3D& translation, const datatypes::Rotation3D& rotation,
            const datatypes::Scale3D& scale
        ) {
            return Transform3D(translation, rotation, scale, false);
        }

        /// Creates a new rigid transform (translation & rotation only).
        ///
        /// \param _translation \copydoc Transform3D::translation
        /// \param rotation \copydoc datatypes::TranslationRotationScale3D::rotation
        /// \param from_parent \copydoc datatypes::TranslationRotationScale3D::from_parent
        Transform3D(
            const components::Translation3D& _translation, const datatypes::Rotation3D& rotation,
            bool from_parent = false
        )
            : transform(datatypes::TranslationRotationScale3D(rotation, from_parent)),
              translation(Collection<components::Translation3D>::take_ownership(_translation)) {}

        /// From a rotation & scale.
        ///
        /// \param translation \copydoc Transform3D::translation
        /// \param rotation \copydoc datatypes::TranslationRotationScale3D::rotation
        static Transform3D from_translation_rotation(
            const components::Translation3D& translation, const datatypes::Rotation3D& rotation
        ) {
            return Transform3D(translation, rotation, false);
        }

        /// From translation & scale only.
        ///
        /// \param _translation \copydoc Transform3D::translation
        /// \param scale datatypes::TranslationRotationScale3D::scale
        /// \param from_parent \copydoc datatypes::TranslationRotationScale3D::from_parent
        Transform3D(
            const components::Translation3D& _translation, const datatypes::Scale3D& scale,
            bool from_parent = false
        )
            : transform(datatypes::TranslationRotationScale3D(scale, from_parent)),
              translation(Collection<components::Translation3D>::take_ownership(_translation)) {}

        /// From a translation applied after a scale.
        ///
        /// \param translation \copydoc Transform3D::translation
        /// \param scale datatypes::TranslationRotationScale3D::scale
        static Transform3D from_translation_scale(
            const components::Translation3D& translation, const datatypes::Scale3D& scale
        ) {
            return Transform3D(translation, scale, false);
        }

        /// From translation & uniform scale only.
        ///
        /// \param _translation \copydoc Transform3D::translation
        /// \param uniform_scale Uniform scale factor that is applied to all axis equally.
        /// \param from_parent \copydoc datatypes::TranslationRotationScale3D::from_parent
        ///
        /// _Implementation note:_ This explicit overload prevents interpretation of the float as
        /// bool, leading to a call to the wrong overload.
        Transform3D(
            const components::Translation3D& _translation, float uniform_scale,
            bool from_parent = false
        )
            : transform(datatypes::TranslationRotationScale3D(uniform_scale, from_parent)),
              translation(Collection<components::Translation3D>::take_ownership(_translation)) {}

        /// From rotation & scale.
        ///
        /// \param rotation \copydoc datatypes::TranslationRotationScale3D::rotation
        /// \param scale datatypes::TranslationRotationScale3D::scale
        /// \param from_parent \copydoc datatypes::TranslationRotationScale3D::from_parent
        Transform3D(
            const datatypes::Rotation3D& rotation, const datatypes::Scale3D& scale,
            bool from_parent = false
        )
            : transform(datatypes::TranslationRotationScale3D(rotation, scale, from_parent)) {}

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
            : transform(datatypes::TranslationRotationScale3D(rotation, uniform_scale, from_parent)
              ) {}

        /// From a rotation & scale.
        ///
        /// \param rotation \copydoc datatypes::TranslationRotationScale3D::rotation
        /// \param scale datatypes::TranslationRotationScale3D::scale
        static Transform3D from_rotation_scale(
            const datatypes::Rotation3D& rotation, const datatypes::Scale3D& scale
        ) {
            return Transform3D(rotation, scale, false);
        }

        /// From rotation only.
        ///
        /// \param rotation \copydoc datatypes::TranslationRotationScale3D::rotation
        /// \param from_parent \copydoc datatypes::TranslationRotationScale3D::from_parent
        Transform3D(const datatypes::Rotation3D& rotation, bool from_parent = false)
            : transform(datatypes::TranslationRotationScale3D(rotation, from_parent)) {}

        /// From rotation only.
        ///
        /// \param rotation \copydoc datatypes::TranslationRotationScale3D::rotation
        static Transform3D from_rotation(const datatypes::Rotation3D& rotation) {
            return Transform3D(rotation, false);
        }

        /// From scale only.
        ///
        /// \param scale \copydoc datatypes::TranslationRotationScale3D::from_parent
        /// \param from_parent \copydoc datatypes::TranslationRotationScale3D::scale
        Transform3D(const datatypes::Scale3D& scale, bool from_parent = false)
            : transform(datatypes::TranslationRotationScale3D(scale, from_parent)) {}

        /// From scale only.
        ///
        /// \param scale datatypes::TranslationRotationScale3D::scale
        static Transform3D from_scale(const datatypes::Scale3D& scale) {
            return Transform3D(scale, false);
        }

        /// TODO(#6831): Should be autogenerated once from_parent component is introduced
        Transform3D with_from_parent(bool from_parent) && {
            auto translation_rotation_scale = transform.repr.get_translation_rotation_scale();
            if (translation_rotation_scale != nullptr) {
                auto cpy = *translation_rotation_scale;
                cpy.from_parent = from_parent;
                transform = cpy;
            }
            // See: https://github.com/rerun-io/rerun/issues/4027
            RR_WITH_MAYBE_UNINITIALIZED_DISABLED(return std::move(*this);)
        }

        // </CODEGEN_COPY_TO_HEADER>
#endif

    } // namespace archetypes
} // namespace rerun
