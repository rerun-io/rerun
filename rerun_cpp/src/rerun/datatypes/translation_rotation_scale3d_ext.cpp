#include "translation_rotation_scale3d.hpp"

// <CODEGEN_COPY_TO_HEADER>
#include "../warning_macros.hpp"

// </CODEGEN_COPY_TO_HEADER>
namespace rerun {
    namespace datatypes {

#if 0
            // <CODEGEN_COPY_TO_HEADER>

            static const TranslationRotationScale3D IDENTITY;

            // Need to disable the maybe-uninitialized here because the compiler gets confused by the combination
            // of union-types datatypes inside of an optional component.
            //
            // See: https://github.com/rerun-io/rerun/issues/4027
            DISABLE_MAYBE_UNINITIALIZED_PUSH
            TranslationRotationScale3D(const TranslationRotationScale3D& other)
                : translation(other.translation),
                  rotation(other.rotation),
                  scale(other.scale),
                  from_parent(other.from_parent) {}

            DISABLE_MAYBE_UNINITIALIZED_POP

            /// Creates a new 3D transform from translation/rotation/scale.
            ///
            /// \param from_parent_ If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            /// parent.
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
            /// \param from_parent_ If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            /// parent.
            TranslationRotationScale3D(
                const Vec3D& translation_, const Rotation3D& rotation_, bool from_parent_ = false
            )
                : translation(translation_),
                  rotation(rotation_),
                  scale(std::nullopt),
                  from_parent(from_parent_) {}

            /// From translation & scale only.
            ///
            /// \param from_parent_ If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            /// parent.
            TranslationRotationScale3D(
                const Vec3D& translation_, const Scale3D& scale_, bool from_parent_ = false
            )
                : translation(translation_),
                  rotation(std::nullopt),
                  scale(scale_),
                  from_parent(from_parent_) {}

            /// From rotation & scale only.
            ///
            /// \param rotation_ 3D rotation.
            /// \param scale_ 3D scale.
            /// \param from_parent_ If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            /// parent.
            TranslationRotationScale3D(
                const Rotation3D& rotation_, const Scale3D& scale_, bool from_parent_ = false
            )
                : translation(std::nullopt),
                  rotation(rotation_),
                  scale(scale_),
                  from_parent(from_parent_) {}

            /// From translation only.
            ///
            /// \param rotation_ 3D translation.
            /// \param from_parent_ If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            /// parent.
            TranslationRotationScale3D(const Vec3D& translation_, bool from_parent_ = false)
                : translation(translation_),
                  rotation(std::nullopt),
                  scale(std::nullopt),
                  from_parent(from_parent_) {}

            /// From rotation only.
            ///
            /// \param rotation_ 3D rotation.
            /// \param from_parent_ If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            /// parent.
            TranslationRotationScale3D(const Rotation3D& rotation_, bool from_parent_ = false)
                : translation(std::nullopt),
                  rotation(rotation_),
                  scale(std::nullopt),
                  from_parent(from_parent_) {}

            /// From scale only.
            ///
            /// \param rotation_ 3D scale.
            /// \param from_parent_ If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            /// parent.
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
