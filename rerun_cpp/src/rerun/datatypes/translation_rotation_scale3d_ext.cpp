#include "translation_rotation_scale3d.hpp"

// <CODEGEN_COPY_TO_HEADER>
#include "../warning_macros.hpp"

// </CODEGEN_COPY_TO_HEADER>
namespace rerun {
    namespace datatypes {

#if 0
            // <CODEGEN_COPY_TO_HEADER>

            /// Identity transformation.
            ///
            /// Applying this transform does not alter an entities translation/rotation/scale.
            static const TranslationRotationScale3D IDENTITY;

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
