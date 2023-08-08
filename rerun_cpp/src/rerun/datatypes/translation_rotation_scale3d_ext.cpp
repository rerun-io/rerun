#include "translation_rotation_scale3d.hpp"

// Uncomment for better auto-complete while editing the extension.
// #define EDIT_EXTENSION

namespace rerun {
    namespace datatypes {

#ifdef EDIT_EXTENSION
        struct TranslationRotationScale3DExt {
            std::optional<Vec3D> translation;
            std::optional<Rotation3D> rotation;
            std::optional<Scale3D> scale;
            bool from_parent;

#define TranslationRotationScale3D TranslationRotationScale3DExt
            // [CODEGEN COPY TO HEADER START]

            static const TranslationRotationScale3D IDENTITY;

            /// Creates a new 3D transform from translation/rotation/scale.
            ///
            /// @param _from_parent If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            /// parent.
            TranslationRotationScale3D(
                const std::optional<Vec3D>& _translation,
                const std::optional<Rotation3D>& _rotation, const std::optional<Scale3D>& _scale,
                bool _from_parent
            )
                : translation(_translation),
                  rotation(_rotation),
                  scale(_scale),
                  from_parent(_from_parent) {}

            /// Creates a new 3D transform from translation/rotation/scale.
            ///
            /// @param _from_parent If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            TranslationRotationScale3D(
                const Vec3D& _translation, const Rotation3D& _rotation, const Scale3D& _scale,
                bool _from_parent = false
            )
                : translation(_translation),
                  rotation(_rotation),
                  scale(_scale),
                  from_parent(_from_parent) {}

            /// Creates a new rigid transform (translation & rotation only).
            ///
            /// @param _from_parent If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            TranslationRotationScale3D(
                const Vec3D& _translation, const Rotation3D& _rotation, bool _from_parent = false
            )
                : translation(_translation),
                  rotation(_rotation),
                  scale(std::nullopt),
                  from_parent(_from_parent) {}

            /// From translation & scale only.
            ///
            /// @param _from_parent If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            TranslationRotationScale3D(
                const Vec3D& _translation, const Scale3D& _scale, bool _from_parent = false
            )
                : translation(_translation),
                  rotation(std::nullopt),
                  scale(_scale),
                  from_parent(_from_parent) {}

            /// From translation only.
            ///
            /// @param from_parent If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            TranslationRotationScale3D(
                const datatypes::Vec3D& _translation, bool _from_parent = false
            )
                : translation(_translation),
                  rotation(std::nullopt),
                  scale(std::nullopt),
                  from_parent(_from_parent) {}

            /// From rotation only.
            ///
            /// @param from_parent If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            TranslationRotationScale3D(
                const datatypes::Rotation3D& _rotation, bool _from_parent = false
            )
                : translation(std::nullopt),
                  rotation(_rotation),
                  scale(std::nullopt),
                  from_parent(_from_parent) {}

            /// From scale only.
            ///
            /// @param from_parent If true, the transform maps from the parent space to the space
            /// where the transform was logged. Otherwise, the transform maps from the space to its
            TranslationRotationScale3D(const datatypes::Scale3D& _scale, bool _from_parent = false)
                : translation(std::nullopt),
                  rotation(std::nullopt),
                  scale(_scale),
                  from_parent(_from_parent) {}

            // [CODEGEN COPY TO HEADER END]
        };

#undef TranslationAndMat3x3
#else
#define TranslationRotationScale3DExt TranslationRotationScale3D
#endif

        const TranslationRotationScale3DExt TranslationRotationScale3DExt::IDENTITY =
            TranslationRotationScale3D();

    } // namespace datatypes
} // namespace rerun
