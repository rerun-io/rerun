#include "translation_rotation_scale3d.hpp"

// <CODEGEN_COPY_TO_HEADER>
#include "../compiler_utils.hpp"

// </CODEGEN_COPY_TO_HEADER>
namespace rerun {
    namespace datatypes {

#if 0
            // <CODEGEN_COPY_TO_HEADER>

            /// Identity transformation.
            ///
            /// Applying this transform does not alter an entity's transformation.
            /// It has all optional fields set to `std::nullopt`.
            static const TranslationRotationScale3D IDENTITY;

            // Need to disable the maybe-uninitialized here because the compiler gets confused by the combination
            // of union-types datatypes inside of an optional component.
            //
            // See: https://github.com/rerun-io/rerun/issues/4027
            RERUN_DISABLE_MAYBE_UNINITIALIZED_PUSH
            TranslationRotationScale3D(const TranslationRotationScale3D& other)
                : translation(other.translation),
                  rotation(other.rotation),
                  scale(other.scale),
                  from_parent(other.from_parent) {}

            RR_DISABLE_MAYBE_UNINITIALIZED_POP

            /// Creates a new 3D transform from translation/rotation/scale.
            ///
            /// \param translation_ \copydoc TranslationRotationScale3D::translation
            /// \param rotation_ \copydoc TranslationRotationScale3D::rotation
            /// \param scale_ \copydoc TranslationRotationScale3D::scale
            /// \param from_parent_ \copydoc TranslationRotationScale3D::from_parent
            TranslationRotationScale3D(
                const std::optional<Vec3D>& translation_,
                const std::optional<Rotation3D>& rotation_, const std::optional<Scale3D>& scale_,
                bool from_parent_ = false
            )
                : translation(translation_),
                  rotation(rotation_),
                  scale(scale_),
                  from_parent(from_parent_) {}

            /// Creates a new 3D transform from translation/rotation/uniform-scale.
            ///
            /// \param translation_ \copydoc TranslationRotationScale3D::translation
            /// \param rotation_ \copydoc TranslationRotationScale3D::rotation
            /// \param uniform_scale Uniform scale factor that is applied to all axis equally.
            /// \param from_parent_ \copydoc TranslationRotationScale3D::from_parent
            ///
            /// _Implementation note:_ This explicit overload prevents interpretation of the float as
            /// bool, leading to a call to the wrong overload.
            TranslationRotationScale3D(
                const Vec3D& translation_, const Rotation3D& rotation_,
                float uniform_scale, bool from_parent_ = false
            )
                : translation(translation_),
                  rotation(rotation_),
                  scale(uniform_scale),
                  from_parent(from_parent_) {}

            /// Creates a new rigid transform (translation & rotation only).
            ///
            /// \param translation_ \copydoc TranslationRotationScale3D::translation
            /// \param rotation_ \copydoc TranslationRotationScale3D::rotation
            /// \param from_parent_ \copydoc TranslationRotationScale3D::from_parent
            TranslationRotationScale3D(
                const Vec3D& translation_, const Rotation3D& rotation_, bool from_parent_ = false
            )
                : translation(translation_),
                  rotation(rotation_),
                  scale(std::nullopt),
                  from_parent(from_parent_) {}

            /// From translation & scale only.
            ///
            /// \param translation_ \copydoc TranslationRotationScale3D::translation
            /// \param scale_ \copydoc TranslationRotationScale3D::scale
            /// \param from_parent_ \copydoc TranslationRotationScale3D::from_parent
            TranslationRotationScale3D(
                const Vec3D& translation_, const Scale3D& scale_, bool from_parent_ = false
            )
                : translation(translation_),
                  rotation(std::nullopt),
                  scale(scale_),
                  from_parent(from_parent_) {}

            /// From translation & uniform scale.
            ///
            /// \param translation_ \copydoc TranslationRotationScale3D::translation
            /// \param uniform_scale Uniform scale factor that is applied to all axis equally.
            /// \param from_parent_ \copydoc TranslationRotationScale3D::from_parent
            ///
            /// _Implementation note:_ This explicit overload prevents interpretation of the float as
            /// bool, leading to a call to the wrong overload.
            TranslationRotationScale3D(
                const Vec3D& translation_, float uniform_scale, bool from_parent_ = false
            )
             : translation(translation_),
                  rotation(std::nullopt),
                  scale(uniform_scale),
                  from_parent(from_parent_) {}

            /// From rotation & scale only.
            ///
            /// \param rotation_ \copydoc TranslationRotationScale3D::rotation
            /// \param scale_ \copydoc TranslationRotationScale3D::scale
            /// \param from_parent_ \copydoc TranslationRotationScale3D::from_parent
            TranslationRotationScale3D(
                const Rotation3D& rotation_, const Scale3D& scale_, bool from_parent_ = false
            )
                : translation(std::nullopt),
                  rotation(rotation_),
                  scale(scale_),
                  from_parent(from_parent_) {}

            /// From rotation & uniform scale.
            ///
            /// \param rotation_ \copydoc TranslationRotationScale3D::rotation
            /// \param uniform_scale Uniform scale factor that is applied to all axis equally.
            /// \param from_parent_ \copydoc TranslationRotationScale3D::from_parent
            ///
            /// _Implementation note:_ This explicit overload prevents interpretation of the float as
            /// bool, leading to a call to the wrong overload.
            TranslationRotationScale3D(
                const Rotation3D& rotation_, float uniform_scale, bool from_parent_ = false
            )
             : translation(std::nullopt),
                  rotation(rotation_),
                  scale(uniform_scale),
                  from_parent(from_parent_) {}

            /// From translation only.
            ///
            /// \param translation_ 3D translation.
            /// \param from_parent_
            TranslationRotationScale3D(const Vec3D& translation_, bool from_parent_ = false)
                : translation(translation_),
                  rotation(std::nullopt),
                  scale(std::nullopt),
                  from_parent(from_parent_) {}

            /// From rotation only.
            ///
            /// \param rotation_ \copydoc TranslationRotationScale3D::rotation
            /// \param from_parent_ \copydoc TranslationRotationScale3D::from_parent
            TranslationRotationScale3D(const Rotation3D& rotation_, bool from_parent_ = false)
                : translation(std::nullopt),
                  rotation(rotation_),
                  scale(std::nullopt),
                  from_parent(from_parent_) {}


            /// From scale only.
            ///
            /// \param scale_ \copydoc TranslationRotationScale3D::scale
            /// \param from_parent_ \copydoc TranslationRotationScale3D::from_parent
            TranslationRotationScale3D(const Scale3D& scale_, bool from_parent_ = false)
                : translation(std::nullopt),
                  rotation(std::nullopt),
                  scale(scale_),
                  from_parent(from_parent_) {}

            // </CODEGEN_COPY_TO_HEADER>
        };
#endif

        const TranslationRotationScale3D TranslationRotationScale3D::IDENTITY =
            TranslationRotationScale3D();

    } // namespace datatypes
} // namespace rerun
